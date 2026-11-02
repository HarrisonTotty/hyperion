/**
 * A body's terrain pass (plan R05, R05.T11.c, Design notes 6, 7, 10, 17 and 23; shared by the
 * views from plan R10, R10.T16.b): selection (on its own cadence), the patch cache, the
 * height-worker pool and the draw set drive each view's instanced draw of T11.a's resources with
 * T11.b's material.
 *
 * @remarks
 * One pass draws one body, in one shading, for every view that hands the body over
 * (decision-r10-live-path, D3): {@link TerrainPass.step} selects once over all of them, each with
 * its own τ, presented size and weight (R05 Design notes 23 and 24), over one cache and one lane of
 * the pool, and {@link TerrainPass.drawFor} gives each view its own draw: its instance records (the
 * draw set's patches it sees, relative to its own camera, at its own morph bands), its own contact
 * records and lights, and its own annunciation's debounce. {@link TerrainPass.frame} is the descent
 * spike's single view, a step and a draw, as R05 built it.
 *
 * Selection is pure and costly, so the pass re-runs it only when its inputs move enough
 * (decision-r05-patch-demand.md, 4d): on a change of the views, setting, viewport, field of view,
 * contacts, baked height ranges, level table or survey; an orientation change of more than one
 * pixel's angle in any view; or a camera move of more than {@link RESELECT_MOVE_FRACTION} = m ÷
 * (1 + m), for m = `SELECTION_MARGIN`, of the box distance to the nearest selected patch the view
 * sees that is not at the finest level (with none, the view's own nearest surveyed ground,
 * `Selection.nearestSurveyByViewM`). It selects at τ ÷ (1 + m), so that the drawn error stays within
 * τ between runs, and with a budget of half the cache's slots (4b). Under
 * {@link TerrainPassOptions.frustumMargin}'s `motion`, the terrain session's, each view's frustum
 * is opened for selection by its motion ({@link frustumMarginRad}). The morph bands are computed at
 * τ, raised where the budget binds ({@link effectiveTauPx}). The draw set is resolved when
 * selection runs, in place by one `DrawSetResolver` per cache, with its morph holds at its
 * coarse–fine edges and corners (`CoarseFineHolds`, R10.T8.c), the same for every view, and each
 * view's instance and contact records are written every frame into buffers made once.
 *
 * A body's pass follows its field as it grows (R10 Design note 16, as T6.b and T8.b built the
 * rebake rule): {@link TerrainPass.setLevels} takes each block's level table, and
 * {@link TerrainPass.fieldChanged} each block's change, asking again, at its last priority, for each
 * resident patch the rule names while its old bake stays drawn, and telling the caller once those
 * bakes have landed, so that the body's GPU coverage mask takes the block's cells only then
 * (decision D7). Given the body's survey ({@link TerrainPassOptions.survey}), selection leaves the
 * bare ground to the reference surface (D6). Once every height worker is lost, the pass asks for
 * nothing more and draws nothing, and each view's frame carries the fault
 * ({@link TERRAIN_NOT_AVAILABLE}); likewise once its session halts it ({@link TerrainPass.halt}),
 * with the cause the session gives (plan R10, R10.T16.e).
 *
 * Positions follow R02's differencing: patch origins and contacts are body-fixed `f64`, rotated
 * into the body's non-rotating axes, less the camera in those axes, and narrowed once into the
 * records; the view's rotation is the camera's orientation in the body's axes.
 *
 * A body's patches baked with their class weights take R10's lit material (R10.T10.b,
 * `terrainMaterial.ts`), lit by the frame's {@link TerrainFrameInput.lighting}; a body's baked
 * without them take the wireframe's depth with its contours and graticule (R10.T11,
 * `terrainWire.ts`), drawn by each frame's {@link TerrainFrameInput.wire}; the test planet keeps
 * R05's material and its one sun.
 *
 * {@link TerrainPass.dispose} releases every handle the pass made on the current device: its
 * resources, every view's, its mesh, its materials and its tables of no air
 * (decision-r10-t16-contracts §1).
 *
 * For a view's frame (plan R10, R10.T16.c) a lit pass also gives the terrain as depth alone, for
 * the symbology's pass over the image ({@link TerrainPass.symbologyDepthMaterial}, drawn with a
 * view's draw's resources), whether a view's frame may give the body's coverage mask before its
 * draw ({@link TerrainPass.maskReadyFor}), and the heights of its resident bakes that the survey
 * edge's rim and its note's line of sight read ({@link TerrainPass.heights}).
 *
 * On the high setting a lit pass also draws each view's shadow cascades (plan R10, R10.T9.b;
 * Design note 11; decision-r10-t9b-design), for the body that takes them in the view's frame
 * ({@link TerrainFrameInput.takesCascades}): {@link TerrainPass.drawFor} fits the four cascades to
 * the view (R10.T9.a's `fitCascades`, the view's snapped centres carried from its frame before)
 * and to the top edge of star 0's disc (decision-r10-t9b-penumbra), so that their shadow begins
 * where the horizon map's share m reaches 0 and min(m, c′) is m across the penumbra (non-terrain
 * casters' shadows are short by about half a penumbra, ρL, 4.7 cm at 10 m), culls the view's own
 * records into each cascade by its light-space box and T9.a's caster reach,
 * and renders the four caster passes into the session's one set ({@link TerrainShadows}) before
 * it returns, so that they are submitted before the bodies pass its draw is drawn in; the lit draw
 * binds the set, or the placeholder where none exists, and receives the cascades through its
 * `Draw` ({@link CascadeUniforms}).
 */

import type { Quaternion } from "../camera/pose";
import { conjugate, multiply, quaternionFromRows, rotationRows } from "../camera/quaternion";
import type { ViewId } from "../camera/state";
import type { KernelPair } from "../engine/kernels";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { BucketDrawBase, ClusterBuckets } from "../clusters/buckets";
import {
  CLUSTER_COMPACT,
  COMPACT_PHASES,
  compactWorkgroups,
  packCompactParams,
} from "../clusters/compact";
import { CLUSTER_CULL_KERNEL, CULL_INPUT_WHOLE, receiversRefuse } from "../clusters/cull";
import { CLUSTER_SHAPE_CODE } from "../clusters/record";
import {
  cullHorizonOf,
  type CameraCullPass,
  type CullPass,
  CULL_PARAMS_BYTES,
  CULL_PASS_BYTES,
  MAX_CULL_PASSES,
  packCullParams,
  packCullPasses,
} from "../clusters/cullPass";
import { cascadeCullPasses } from "../clusters/receivers";
import { type ClusterPassStats, ClusterStats } from "../clusters/stats";
import { TERRAIN_PAGE_CLUSTERS } from "../clusters/terrain";
import { TERRAIN_DISCARD_TEMPLATES } from "../clusters/templates";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import type {
  ComputeHandle,
  DepthLayerPass,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  ViewSize,
} from "../engine/types";
import type { BufferHandle } from "../engine/types";
import {
  createAirlessTables,
  surfaceLightingDrawTextures,
  type SurfaceLightingTables,
} from "../atmosphere/surfaceLighting";
import {
  type GeometrySettings,
  type QualitySetting,
  SETTINGS,
  TERRAIN_SETTINGS,
  type TerrainSettings,
} from "../quality/qualitySetting";
import { norm, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  TERRAIN_NOT_AVAILABLE,
  type TerrainAnnunciation,
  TerrainAnnunciationDebounce,
  type TerrainConditions,
  type TerrainFault,
} from "./annunciation";
import { distanceToBoxM, relativeBounds } from "./bounds";
import { type CacheInsert, type DrawSet, DrawSetResolver, PatchCache } from "./cache";
import { CoarseFineHolds, TransitionFades } from "./coarseFineHolds";
import { frustumOf, horizonCone, inFrustum } from "./cull";
import { vertexSpacing } from "./cube";
import { pixelAngleRad } from "./handover";
import { TERRAIN_SHADOW_PASS_LABEL, terrainMaterialSpec } from "./gpu/material";
import {
  CLUSTER_LIT_PASS,
  CLUSTER_OVERLAY_PASS,
  DEFAULT_TERRAIN_VIEW,
  isInteriorBake,
  LIT_INTERIOR_INDICES,
  LIT_PATCH_INDICES,
  PATCH_INDICES,
  TERRAIN_CLUSTER_MESH,
  normalsAtlasOf,
  surveyCellCode,
  type TerrainLayout,
  TerrainResources,
  type TerrainViewBuffers,
} from "./gpu/resources";
import {
  ContactRecords,
  DATUM_SLOT,
  fadeWord,
  INSTANCE_FADES_WORD,
  INSTANCE_PARENT_SLOT_WORD,
  INSTANCE_RECORD_BYTES,
  INSTANCE_SLOT_WORD,
  MIX_STEPS,
  InstanceRecords,
  NO_PARENT_SLOT,
  PARENT_MIX_SLOT,
  parentQuadrant,
  subPatchOf,
  WirePhaseRecords,
} from "./gpu/uniforms";
import { MAX_LEVEL, type PatchKey, parentKey, patchKeyString } from "./patchKey";
import { type PlanetGeometry, planetGeometry, surfacePoint } from "./planet";
import { type MorphSpans, TimeMorph } from "./timeMorph";
import {
  finestPatchSizeM,
  type GroundContact,
  heldGroundUnder,
  heldRadiusM,
  morphRampM,
} from "./grounded";
import {
  MAX_SELECTION_VIEWS,
  type PatchRequest,
  type Selection,
  type SelectionInput,
  selectionErrorM,
  selectPatches,
  splitKeysOf,
  type SurveyLookup,
  type ViewSelectionInput,
} from "./select";
import {
  LOOKAHEAD_COST_STEPS,
  LOOKAHEAD_SAMPLES,
  LOOKAHEAD_SLOW_EVERY,
  lookaheadInput,
  lookaheadSeconds,
  type MovingCamera,
  moving,
  slowSelection,
  timeToNeedS,
  type ViewMotion,
} from "./lookahead";
import { joinDemand } from "./priority";
import { RESELECT_MOVE_FRACTION, selectionTolerancePx } from "./selectionTolerance";
import { PATCH_VERTICES_PER_SIDE, TERRAIN_SHADING_TRAITS, type TerrainShading } from "./slotLayout";
import { ResidentHeights, type ResidentHeightsView } from "./residentHeights";
import { type TerrainMaterialSource, terrainMaterialOf } from "./materialSet";
import {
  type CascadeIndex,
  cascadeUniforms,
  type CascadeUniforms,
  packCascadeUniforms,
} from "./shadows/cascadeShadow";
import {
  type CascadeInput,
  type CascadeSet,
  discTopDirection,
  FarCascadeHolds,
  type FarCascadePlan,
  fitCascades,
} from "./shadows/cascades";
import { CascadeReuse, TerrainShadows } from "./shadows/terrainShadows";
import { SHADOW_CASCADES_TEXTURE, type TerrainLighting } from "./terrainMaterial";
import { CONTOUR_PX, contourPhase, type GraticulePatch, graticulePatch } from "./terrainWire";
import type {
  BakedPatch,
  BakeSettings,
  FieldBlockAnswer,
  HeightSourceSetting,
} from "./workers/messages";
import { draftVertexBits, mergeRefinedNormals } from "./workers/draftNormals";
import type { RefinedDraft, RefineRequest } from "./workers/pool";
import { needsRebake, type RebakeLayers } from "./workers/rebake";

export { DEFAULT_TERRAIN_VIEW } from "./gpu/resources";

/**
 * A view of a pass: a `VIEW` view's own {@link ViewId}, or the descent spike's single view,
 * {@link DEFAULT_TERRAIN_VIEW}, which no `ViewId` can be (the batch review's C4).
 */
export type TerrainViewId = ViewId | typeof DEFAULT_TERRAIN_VIEW;

/**
 * How a pass opens each view's frustum for its selection (plan R10, R10.T16; the batch review's
 * S6): `none`, the camera's own frustum, as R05's spike selects; `motion`, opened by
 * {@link frustumMarginRad}, so that a view turning or moving between selections, or drawing a frame
 * after the step that selected for it, finds its leading edge selected.
 */
export type TerrainFrustumMargin = "none" | "motion";

/** A cluster path's refusal (Design note 16): what was refused, and why. */
function refusalOf(name: string, reason: unknown): string {
  return `${name} refused: ${reason instanceof Error ? reason.message : String(reason)}`;
}

/** The label of the cluster cull's dispatch in `PassTimes` (plan R14, R14.T1.b). */
export const TERRAIN_CLUSTER_CULL_LABEL = "terrain cluster cull";

/** The label of the lit pass's stable compaction's three dispatches (plan R14, R14.T2.a). */
export const TERRAIN_CLUSTER_COMPACT_LABEL = "terrain cluster compact";

/** The cascades' passes' names in the cluster counters (plan R14, R14.T1.b), nearest first. */
export const CASCADE_PASS_NAMES: readonly [string, string, string, string] = [
  "cascade 1",
  "cascade 2",
  "cascade 3",
  "cascade 4",
];

/**
 * The most a view's frustum is opened by under `motion`, rad (about 5.7°): a view that jumps between
 * two steps, as a scene's camera cut does, would otherwise select most of a hemisphere.
 */
export const MAX_FRUSTUM_MARGIN_RAD = 0.1;

/**
 * How far through a level's distance band the CDLOD morph to its parent begins (Design note 6): a
 * hand value, the morph taking the band's outer 30% (Strugar 2009 morphs over an outer part of
 * each range and leaves its size free). T18 may revisit it.
 */
export const MORPH_START_FRACTION = 0.7;

/**
 * The extra skirt margin the height workers bake with, metres: `heightBake.ts`'s `bakeKey` passes
 * none, so selection's inherited ranges reach the skirts' bottoms by the bound and the step alone.
 */
export const WORKER_SKIRT_MARGIN_M = 0;

/** The visual albedo the spike shades with, a hand value (Design note 17). */
export const SPIKE_ALBEDO = 0.15;

/**
 * How a source's patches are shaded: R10's lit material for a body baked with its class weights,
 * the wireframe's depth and contours for a body baked without them (R10.T11; decision D10), and
 * R05's spike shading for the test planet.
 */
export function terrainShadingOf(source: HeightSourceSetting): TerrainShading {
  let shading: TerrainShading;
  switch (source.kind) {
    case "test-planet":
      shading = "spike";
      break;
    case "body":
      switch (source.weights) {
        case "baked":
          shading = "lit";
          break;
        case "skipped":
          shading = "wire";
          break;
      }
      break;
  }
  return shading;
}

/**
 * A wireframe view's contours and graticule over a body's terrain this frame (plan R10, R10.T11;
 * Design note 14), which the wireframe's pass draws with its depth.
 */
export interface TerrainWireFrameInput {
  /** The view's contour interval, m (`terrainWire.ts`' `contourInterval`). */
  readonly intervalM: number;
  /**
   * The field's sea level, m above the datum, where the world has a sea, at which the coastline is
   * drawn; `null` where it has none.
   */
  readonly seaLevelM: number | null;
  /** The device px drawn for each CSS px of a line (`ViewStrokes.strokeScale`). */
  readonly strokeScale: number;
  /**
   * The body's graticule as the view's draw list draws it (R02's `graticule`): its step, rad, and
   * its thin and heavy widths and its casing's on each side, device px; `null` where it draws none.
   */
  readonly graticule: {
    readonly stepRad: number;
    readonly thinPx: number;
    readonly heavyPx: number;
    readonly casingPx: number;
  } | null;
  /** `--text-muted`, `--text` and `--surface-0` as linear colours, alpha 1 (`linearColour`). */
  readonly colours: {
    readonly muted: Float32Array;
    readonly text: Float32Array;
    readonly casing: Float32Array;
  };
  /**
   * The draw list's slope term, device px (`WireframeDrawList.occluderSlopePx`), carried and, since
   * R10.T16.c's grazing fix, unread by `terrainWire.wgsl`, whose depth is pushed by its constant
   * fraction alone.
   */
  readonly occluderSlopePx: number;
}

/** A frame's lights of a lit terrain, with its body's atmosphere tables where it has one. */
export interface TerrainFrameLighting extends TerrainLighting {
  /** The body's tables (`surfaceLightingTablesOf`); absent for a body without an atmosphere. */
  readonly atmosphere?: SurfaceLightingTables;
}

/**
 * The height pool the pass drives: a lane of T10's `HeightWorkerPool` (`HeightWorkerPool.lane`,
 * R10.T16.b), the descent spike's pool of one lane, or a test's fake.
 */
export interface TerrainPool {
  reprioritise(demand: ReadonlyArray<PatchRequest>): void;
  onBaked(cb: (bake: BakedPatch) => void): () => void;
  terminate(): void;
  /**
   * Listens for the requests given up because their bake failed (R10.T16.b: a rebake given up
   * leaves its old bake drawn); absent, none is told.
   */
  onFailed?(cb: (failure: { readonly keyString: string }) => void): () => void;
  /**
   * Whether the pool can bake nothing more, every worker lost (R10.T16.b's fault); absent, it is
   * never so, as for the descent spike's pool.
   */
  readonly unavailable?: boolean;
  /**
   * Asks for the normals a drafted bake left between its vertices ({@link BakedPatch.drafted};
   * R10.T6.a's progressive bake), at a lower priority than any bake; absent, a draft stays drawn.
   */
  refine?(r: RefineRequest): void;
  /** Drops a refinement asked, whose answer is then not told. */
  dropRefinement?(keyString: string): void;
  /** Listens for refined normals; returns the function that stops listening. */
  onRefined?(cb: (refined: RefinedDraft) => void): () => void;
  /** The refinements asked and not yet told; absent, none. */
  readonly refiningCount?: number;
}

/**
 * Makes the pass's pool for the layout's bake settings; called again after a device rebuild: the
 * terrain session's gives a new lane of its one pool (R10.T16.b), so that no field is posted again.
 */
export type TerrainPoolFactory = (bake: BakeSettings) => TerrainPool;

/** The view the pass draws, in the body's non-rotating axes. */
export interface TerrainView {
  /** The body's rotation, body-fixed axes into its non-rotating axes. */
  readonly rotation: Rotation3;
  /** The camera less the body's centre, in the body's non-rotating axes, metres (`f64`). */
  readonly cameraM: Vec3;
  /** The camera's orientation in the body's non-rotating axes. */
  readonly orientation: Quaternion;
  readonly fovXRad: number;
  /**
   * The size it selects at: the presented size, device px, ÷ max(1, the display's device-pixel
   * ratio), its CSS px (`selectionViewport`; R05 Design note 23).
   */
  readonly viewport: ViewSize;
}

/** One frame's inputs. */
export interface TerrainFrameInput {
  readonly view: TerrainView;
  /** Grounded and descending bodies, body-fixed (Design note 9). */
  readonly grounded: ReadonlyArray<GroundContact>;
  /** The unit direction towards the sun, body-fixed. */
  readonly sunDirectionBodyFixed: Vec3;
  /** The sun's illuminance per Rec. 709 channel at the body, lux (`solar.ts`, scaled by 1 ÷ d²). */
  readonly sunIlluminanceLx: readonly [number, number, number];
  /** The frame's pre-exposure (R02's `exposureScale` of its EV100). */
  readonly exposureScale: number;
  /**
   * What lights a lit terrain (R10.T10.b): the body's stars, occluders and planetshine sources,
   * body-fixed, its uniform law and the frame's pre-exposure, with its atmosphere's tables where it
   * has one. R05's spike shading reads the sun above instead; a lit terrain given none draws its
   * patches unlit.
   */
  readonly lighting?: TerrainFrameLighting;
  /**
   * What the wireframe's terrain draws over its depth (R10.T11): its contours and graticule. A
   * wireframe pass given none draws nothing; the other shadings read none.
   */
  readonly wire?: TerrainWireFrameInput;
  /**
   * Whether this body takes the view's shadow cascades this frame (plan R10, R10.T9.b;
   * decision-r10-t9b-design D3): one set serves one body a view-frame, the handed-over body over
   * whose datum the camera stands lowest (`LiveFrameTerrain.draws`). Read by a lit pass on the high
   * setting alone; absent, it does not, and the body has the horizon map alone.
   */
  readonly takesCascades?: boolean;
  /** A monotonic time for the annunciation's debounce, milliseconds. */
  readonly nowMs: number;
}

/** One view-frame's shadow cascades (plan R10, R10.T9.b), as {@link TerrainPass.drawFor} drew them. */
export interface TerrainCascadeFrame {
  /** The view's four cascades this frame (R10.T9.a's `fitCascades`), with their weight w. */
  readonly set: CascadeSet;
  /**
   * The cascades the frame's receivers sample (plan R14, R14.T6.b): `set`, with each far cascade it
   * holds from an earlier frame as that frame drew it (`heldCascadeFor`); a receiver that is not
   * the terrain's samples the layers through these, not through `set`.
   */
  readonly received: CascadeSet;
  /**
   * The caster passes rendered into the set this frame, one a cascade rendered, nearest first, each
   * the terrain's draw of the view's records in that cascade (where it has any) and then the other
   * casters' ({@link TerrainShadows.casters}); none at w = 0, before the caster material is
   * compiled, or with no draw of the view's terrain, when the lit draw samples nothing. A far
   * cascade the frame holds from an earlier one has none (plan R14, R14.T6.b;
   * `GeometrySettings.farCascadeRate`), so that at a rate of 4 a held view's frames render 4, 2, 3,
   * 2, 3… passes.
   */
  readonly passes: ReadonlyArray<DepthLayerPass>;
  /**
   * Each cascade's patches cast by the lit mesh this frame, nearest first (10,752 triangles each):
   * the edge patches', and the interior patches' off the cluster path, whose own counts are the
   * cluster counters' ({@link TerrainPass.clusterStats}; plan R14, R14.T1.b).
   */
  readonly litMeshCasters: readonly [number, number, number, number];
  /** Whether the interior patches cast through the cluster path this frame (R14.T1.b). */
  readonly clustered: boolean;
  /**
   * The cascades this frame reused, nearest first (plan R14, R14.T6.a): every input of their
   * casters as at the view's last render of their layer, which it rendered last, so that they take
   * no pass and keep their layers.
   */
  readonly reused: readonly [boolean, boolean, boolean, boolean];
}

/** One frame's outcome. */
export interface TerrainFrame {
  /**
   * Its instanced draws, in order ({@link TerrainViewFrame.draws}); none before the material is
   * ready or with nothing to draw.
   */
  readonly draws: ReadonlyArray<DrawItem>;
  /**
   * The pass's one draw set, rewritten in place when selection next runs: read it before the next
   * {@link TerrainPass.frame} call, or copy it.
   */
  readonly drawSet: DrawSet;
  readonly selection: Selection;
  /** Whether this frame ran selection. */
  readonly reselected: boolean;
  readonly conditions: TerrainConditions;
  /** The debounced label line (Design note 23), for `labelStatements`. */
  readonly annunciation: TerrainAnnunciation | null;
}

/** How many patches a view's draws draw (plan R14, R14.T1.a; {@link TerrainViewFrame.patches}). */
export interface TerrainDrawnPatches {
  /** The interior patches, drawn by the lit pass's interior mesh. */
  readonly interior: number;
  /** The others, drawn by the shading's own mesh, or {@link TerrainDrawnPatches.fins}. */
  readonly edge: number;
  /**
   * Of the others, those drawn through the lit pass's cluster path beside the interior patches
   * (plan R14, R14.T1.g; `GeometrySettings.finClusters`), in every pass.
   */
  readonly fins: number;
}

/** No patches drawn. */
const NO_PATCHES: TerrainDrawnPatches = { interior: 0, edge: 0, fins: 0 };

/**
 * The error unit an edge patch's record takes on the cluster path (plan R14, R14.T1.g), m: past any
 * cascade's texel, so that its clusters cast at stride 1, as its lit mesh does.
 */
const FIN_ERROR_UNIT_M = 1e30;

/**
 * A view's triangles at its last draw, by pass (plan R14, R14.T1.b; {@link TerrainPass.counts}):
 * what the terrain run's and the descent spike's results report beside their GPU times.
 */
export interface TerrainGeometryCounts {
  /** Its draws' patches, the interior patches' and the others'. */
  readonly patches: TerrainDrawnPatches;
  /**
   * Its draws' triangles: the interior patches' 8,704 each, the others' their mesh's, those on the
   * cluster path (plan R14, R14.T1.g) as their lit mesh's.
   */
  readonly drawnTriangles: number;
  /**
   * On the lit pass's cluster path (plan R14, R14.T2.a) its draws' triangles as drawn: the edge
   * patches' and the lit pass's counted clusters', from the last sample back; `null` off that path
   * and before a sample, where {@link TerrainGeometryCounts.drawnTriangles} is the interior mesh's.
   */
  readonly litClusterTriangles: number | null;
  /**
   * Each cascade's caster triangles, nearest first: the lit mesh's casters' 10,752 each and the
   * cluster path's counted triangles, from the last sample to have come back; `null` where the
   * frame drew no cascades, or cast through the cluster path with no sample back yet.
   */
  readonly casterTriangles: readonly [number, number, number, number] | null;
  /**
   * The overlay's terrain depth's triangles (plan R14, R14.T1.e): by the way back its draws', the
   * drawn triangles; on the cluster path the edge patches' and the counted clusters', from the last
   * sample back, `null` before one; none of them where the overlay takes the scene's depth, which
   * the photorealistic renderer decides.
   */
  readonly overlayTriangles: number | null;
}

/** One view that hands the pass's body over, as a step takes it (R10.T16.b). */
export interface TerrainPassView {
  /** The view's own name, which its draw ({@link TerrainPass.drawFor}) and its buffers go by. */
  readonly id: ViewId;
  /** Its camera, from the body, as it last drew. */
  readonly view: TerrainView;
  /**
   * Its tolerance τ, pixels: its style's on the setting (T10.e's `handoverTauPx`). Selection runs
   * at τ ÷ (1 + m), its morph bands at τ.
   */
  readonly tauPx: number;
  /**
   * Its streaming weight (R05 Design note 24): `PRIMARY_VIEW_WEIGHT` or `SECONDARY_VIEW_WEIGHT`,
   * from its budget's `streamPriority`.
   */
  readonly weight: number;
}

/** A view as a step takes it: a {@link TerrainPassView}, or the spike's default view. */
type StepView = Omit<TerrainPassView, "id"> & { readonly id: TerrainViewId };

/** One step's inputs: the views that hand the body over, and the grounded bodies. */
export interface TerrainStepInput {
  /** At most {@link MAX_SELECTION_VIEWS}, in a stable order. */
  readonly views: ReadonlyArray<TerrainPassView>;
  /** Grounded and descending bodies, body-fixed (Design note 9). */
  readonly grounded: ReadonlyArray<GroundContact>;
}

/** One step's outcome, which every view's draw of the frame reads. */
export interface TerrainStep {
  /** The pass's one draw set, rewritten in place when selection next runs. */
  readonly drawSet: DrawSet;
  readonly selection: Selection;
  /** Whether this step ran selection. */
  readonly reselected: boolean;
}

/** One view's frame of a step ({@link TerrainPass.drawFor}): a frame's inputs but its contacts. */
export type TerrainViewFrameInput = Omit<TerrainFrameInput, "grounded">;

/** One view's frame of the pass (R10.T16.b). */
export interface TerrainViewFrame {
  /**
   * Its instanced draws, drawn in order: on a lit pass the interior patches' (plan R14, R14.T1.a),
   * where it sees any and their material is made, then the other patches'; one draw on the other
   * shadings. None before the material is ready, with nothing it sees, or faulted.
   */
  readonly draws: ReadonlyArray<DrawItem>;
  /**
   * How many patches its draws draw (plan R14, R14.T1.a): the interior patches, by their own mesh,
   * and the others, by the shading's; none where it draws nothing.
   */
  readonly patches: TerrainDrawnPatches;
  /** Its conditions, before the debounce. */
  readonly conditions: TerrainConditions;
  /** Its debounced label line (Design note 23), for `labelStatements`; `null` under the fault. */
  readonly annunciation: TerrainAnnunciation | null;
  /**
   * The terrain's fault by the cause the pass was faulted for, every height worker lost
   * ({@link TERRAIN_NOT_AVAILABLE}) or the session's halt (R10.T16.e), for the view's label block's
   * fault line after any graphics fault (decision-r10-t16-contracts §7), or `null`.
   */
  readonly fault: TerrainFault | null;
  /** Whether a selected patch it sees has no resident ancestor, and is not drawn. */
  readonly missing: boolean;
  /** Whether a selected patch it sees is drawn by a coarser resident ancestor. */
  readonly standingIn: boolean;
  /**
   * Whether the body's coverage mask may discard the reference surface in this view (decision D7):
   * no fault, nothing it sees missing, and its draw made wherever it sees a patch, so that the
   * terrain draws all the surveyed ground the mask would discard.
   */
  readonly maskReady: boolean;
  /**
   * The view's shadow cascades this frame (plan R10, R10.T9.b), where its body took them on the
   * high setting under a star; `null` otherwise, when its lit draw takes w = 0.
   */
  readonly cascades: TerrainCascadeFrame | null;
  /**
   * The overlay's terrain depth on the cluster path (plan R14, R14.T1.e;
   * `GeometrySettings.overlayDepth`), where the pass takes it there; `null` where the overlay
   * redraws each of {@link TerrainViewFrame.draws} with the depth material.
   */
  readonly overlayDepth: TerrainOverlayDepth | null;
  /**
   * Whether its draws carry decoration (plan R11, R11.T1; Design note 2), for the view's
   * `DECORATION ON`: what was drawn, not what the setting allows; none is drawn yet.
   */
  readonly decorated: boolean;
}

/**
 * A view-frame's terrain depth for the symbology's pass on the cluster path (plan R14, R14.T1.e):
 * the edge patches' draws, which take the depth material as every draw does by the way back, and
 * the interior patches' bucket draws at stride 1, culled to the camera's frustum, each with the
 * cluster depth (`terrainClusterDepthMaterialSpec`), which pushes its depth as the depth material
 * does; both but the overlay's `occluderSlopePx`, which its frame gives.
 */
export interface TerrainOverlayDepth {
  readonly edge: ReadonlyArray<DrawItem>;
  readonly clusters: ReadonlyArray<DrawItem>;
}

/** The survey a pass's selection reads, with a count of its changes (R10.T16.b). */
export interface TerrainSurveyLookup extends SurveyLookup {
  /** Rises with each change, so that the pass selects again. */
  readonly version: number;
}

/** The pass's options. */
export interface TerrainPassOptions {
  readonly engine: RenderEngine;
  readonly setting: QualitySetting;
  readonly planet: PlanetGeometry;
  /**
   * What the patches' heights come from: the test planet with its ridges, or a body over the
   * field its workers hold (R10.T6.b).
   */
  readonly source: HeightSourceSetting;
  readonly createPool: TerrainPoolFactory;
  /**
   * The terrain's settings, for the spike's variants (T13.c); defaults to
   * `TERRAIN_SETTINGS[setting]`. `setting` still decides the low setting's `DETAIL LIMITED`.
   */
  readonly terrain?: TerrainSettings;
  /**
   * The body's survey (R10.T16.b; `SurveyedCells`), which selection reads so that it never
   * refines, forces, asks for or draws ground with no surveyed cell; absent, every patch is
   * surveyed, as on the test planet.
   */
  readonly survey?: TerrainSurveyLookup;
  /**
   * Whether each selection is recorded as a `terrain.select` `performance.measure` span, for the
   * descent spike's trace (decision-r05-patch-demand.md, 4d). Off by default: each span is an
   * entry the browser keeps, so only the spike's measurement runs pay for it.
   */
  readonly measureSelection?: boolean;
  /** Called with each patch the cache stores and the GPU holds, for the spike's tallies. */
  readonly onResident?: (key: PatchKey) => void;
  /**
   * How each view's frustum is opened for selection ({@link TerrainFrustumMargin}); `none`, the
   * default, keeps R05's selection bit for bit, as the descent spike's `frame` takes it. The terrain
   * session's passes take `motion` (R10.T16; the batch review's S6).
   */
  readonly frustumMargin?: TerrainFrustumMargin;
  /**
   * The time a change of the drawn ground morphs in over, ms (plan R10, R10.T16.i; `timeMorph.ts`):
   * absent, `MORPH_IN_MS`.
   */
  readonly morphInMs?: number;
  /**
   * The budget's hold's margin (plan R10, R10.T16.i; `SelectionInput.budgetHold`): the share of the
   * patch budget the last selection's splits alone may take, so that a pair at the budget's margin
   * does not coarsen and refine by turns. Absent, {@link BUDGET_HOLD_MARGIN}; `null`, no hold, the
   * selection with no history, as before.
   */
  readonly budgetHoldMargin?: number | null;
  /**
   * The level its selection refines seen surveyed ground to at least (`SelectionInput.surveyedFloorLevel`;
   * lane fix-coast-class-map): the session gives a lit pass max(L − 6, 0) under `HANDOVER_AT_CELLS`.
   * Absent, 0: no floor, as before.
   */
  readonly surveyedFloorLevel?: number;
  /**
   * Called with each selection's input and result, right after `selectPatches` and outside the
   * `terrain.select` span, for the spike's second, calibrated pass (T13.c). Unset, it costs nothing.
   */
  readonly onSelect?: (input: SelectionInput, selection: Selection) => void;
  /**
   * The terrain session's materials, compiled when the session was made (plan R10, R10.T14.d;
   * `materialSet.ts`): a lit pass and the wireframe's draw with these, compile none of their own
   * and release none. Absent, as for R05's spike, whose shading never takes them, the pass compiles
   * its own in {@link TerrainPass.ready} and releases them.
   */
  readonly materials?: TerrainMaterialSource;
  /**
   * The terrain session's shadow cascades (plan R10, R10.T9.b; `TerrainShadows`): the one set for
   * the engine, which a lit pass draws each view's cascades into on the high setting, the
   * placeholder its draws bind where no set exists, and the other casters. The pass never lets it
   * go. Absent, a lit pass makes its own, with cascades on the high setting, and lets it go at its
   * disposal.
   */
  readonly shadows?: TerrainShadows;
  /**
   * The body's realised relief range, m (its field header's `realised_relief_m`, R09), read each
   * frame the cascades are fitted, for their caster reach (R10.T9.a's Δh); absent, or `null`, 0.
   */
  readonly reliefM?: () => number | null;
  /**
   * The clock the pass looks ahead of its views' motion by, monotonic milliseconds (plan R10,
   * R10.T14.b; `lookahead.ts`): at each selection it selects again for each view that moves, at its
   * pose L ahead, and asks for that selection's demand at half the view's weight, never drawing it.
   * It times each step's views' velocities and each bake's demand → resident time, L's samples.
   * Absent, the pass looks ahead of nothing and asks for its selection's demand alone, as R05's
   * spike's `frame` does, bit for bit; the terrain session's passes take the session's.
   */
  readonly lookaheadClockMs?: () => number;
  /**
   * How the pass draws its geometry (plan R14): through the cluster path, with each of its levers,
   * or the way back; absent, its setting's (`SETTINGS[setting].geometry`).
   */
  readonly geometry?: GeometrySettings;
  /**
   * The cluster cull's kernel (plan R14, R14.T1.b): `CLUSTER_CULL_KERNEL` where absent. The smoke's
   * `--fixture=broken-wgsl` gives one that does not compile, whose refusal the pass falls back from
   * to today's path, stating it ({@link TerrainPass.clusterRefusal}; Design note 16).
   */
  readonly clusterCullKernel?: KernelPair;
}

/**
 * A listener of a pass's selections and residency beside its options' (plan R10, R10.T14.c): the
 * descent spike's metrics over a terrain session's pass, which the session makes without them
 * ({@link TerrainPass.observe}).
 */
export interface TerrainPassObserver {
  /** Called with each selection, as {@link TerrainPassOptions.onSelect} is. */
  readonly onSelect?: (input: SelectionInput, selection: Selection) => void;
  /** Called with each patch made resident, as {@link TerrainPassOptions.onResident} is. */
  readonly onResident?: (key: PatchKey) => void;
  /**
   * Called with each bake the cache refused a slot (`PatchCache.insert`'s `refused`), which the
   * pass asks for again at its next demand.
   */
  readonly onRefused?: (key: PatchKey) => void;
}

/** The pass's observers while it has none. */
const NO_OBSERVERS: ReadonlyArray<TerrainPassObserver> = [];

/** The `performance.measure` name of one selection, when the pass measures it. */
export const SELECT_MEASURE = "terrain.select";

/** What the pass keeps of a resident patch's bake, for R10's rebake rule (Design note 16). */
interface Resident {
  /** What `needsRebake` reads of its layers; `null` for the test planet's. */
  readonly layers: RebakeLayers | null;
  /** The request it was last asked for with, at whose priority a rebake asks again. */
  readonly request: PatchRequest;
  /**
   * Whether its bake is of survey cell code 0 (`surveyCellCode`: six or more levels coarser than the
   * field) and holds a vertex outside the survey, so that its vertices cannot hold its cells and a
   * view that draws it is given no coverage mask (decision-r10-t16-review §2).
   */
  readonly codeZeroOpen: boolean;
  /**
   * Its bake's origin height h₀ above the datum, m, as its slot record holds it narrowed, from
   * which the wireframe's contour phase is formed (R10.T11; `contourPhase`).
   */
  readonly originHeightM: number;
  /**
   * Whether a lit pass draws it as an interior patch (plan R14, R14.T1.a; `isInteriorBake`): its
   * every vertex surveyed, so that the interior patches' mesh and material, without the skirt
   * heads, the fins and the survey discard, draw what the lit mesh draws. A rebake classifies it
   * again.
   */
  readonly interior: boolean;
  /** Its bake's sizes, which the time morph's spans read (R10.T16.i). */
  readonly sizes: BakeSizes;
}

/**
 * A bake's sizes as the time morph's spans read them (R10.T16.i; `timeMorph.ts`' `MorphSpans`),
 * over its surveyed vertices, the reference surface's elsewhere.
 */
interface BakeSizes {
  /** The largest |h₀ − h₁|, m: how far its morph moves the ground, a refine's or a coarsen's. */
  readonly morphM: number;
  /** The largest |h₀|, m: how far a rise out of the datum moves it. */
  readonly reliefM: number;
  /** h₀ at every {@link SIZE_STRIDE}th vertex each way, m, 0 off the survey: a rebake's change. */
  readonly coarseM: Float32Array;
  /** h₁ there, the same way: the patch's own parent-level surface, which its parent's should be. */
  readonly coarseMorphM: Float32Array;
}

/**
 * An instance's fades word (`fadeWord`) from its own fade's byte and the time morph's heights word
 * for it (`TimeMorph.heightWords`: bits 8 to 31, its rebake's or rise's fade and older slot).
 */
function fadeWordOf(own: number, heights: number): number {
  return fadeWord(own, (heights >>> 8) & MIX_STEPS, heights >>> 20);
}

/** No area was seen at the last step: every patch drawn where nothing was appears at once. */
const NEVER_SEEN = (): boolean => false;

/** The vertices between the samples of {@link BakeSizes.coarseM}, each way. */
const SIZE_STRIDE = 4;

/** The samples of {@link BakeSizes.coarseM} along a side. */
const SIZE_SIDE = (PATCH_VERTICES_PER_SIDE - 1) / SIZE_STRIDE + 1;

/** `bake`'s {@link BakeSizes}. */
function bakeSizesOf(bake: BakedPatch): BakeSizes {
  const mask = bake.layers?.surveyMask;
  const h = bake.heights;
  let morphM = 0;
  let reliefM = 0;
  const coarseM = new Float32Array(SIZE_SIDE * SIZE_SIDE);
  const coarseMorphM = new Float32Array(SIZE_SIDE * SIZE_SIDE);
  for (let y = 0; y < PATCH_VERTICES_PER_SIDE; y += 1) {
    for (let x = 0; x < PATCH_VERTICES_PER_SIDE; x += 1) {
      const v = y * PATCH_VERTICES_PER_SIDE + x;
      if (mask !== undefined && mask[v] !== 255) {
        continue;
      }
      const own = h[2 * v] ?? 0;
      morphM = Math.max(morphM, Math.abs(own - (h[2 * v + 1] ?? 0)));
      reliefM = Math.max(reliefM, Math.abs(own));
      if (x % SIZE_STRIDE === 0 && y % SIZE_STRIDE === 0) {
        const at = (y / SIZE_STRIDE) * SIZE_SIDE + x / SIZE_STRIDE;
        coarseM[at] = own;
        coarseMorphM[at] = h[2 * v + 1] ?? 0;
      }
    }
  }
  return { morphM, reliefM, coarseM, coarseMorphM };
}

/**
 * How far a patch's parent-level surface stands from its parent's own, m (R10.T16.i): the largest
 * |h₁ − the parent's h₀| at the samples the two coarse grids share (every second of the patch's,
 * by its `quadrant` in the parent), each 0 off its own bake's survey. A refine's and a coarsen's
 * heights mix from the parent's surface, and move by this as well as by the patch's own morph
 * where the two bakes disagree (the survey's delta, its edge); 0 where they agree.
 */
function parentDisagreementM(patch: BakeSizes, parent: BakeSizes, quadrant: number): number {
  const half = (SIZE_SIDE - 1) / 2;
  const qx = (quadrant & 1) * half;
  const qy = ((quadrant >> 1) & 1) * half;
  let changeM = 0;
  for (let cy = 0; cy < SIZE_SIDE; cy += 2) {
    for (let cx = 0; cx < SIZE_SIDE; cx += 2) {
      const own = patch.coarseMorphM[cy * SIZE_SIDE + cx] ?? 0;
      const theirs = parent.coarseM[(qy + cy / 2) * SIZE_SIDE + qx + cx / 2] ?? 0;
      changeM = Math.max(changeM, Math.abs(own - theirs));
    }
  }
  return changeM;
}

/** The largest change between two bakes' {@link BakeSizes.coarseM}, m. */
function coarseChangeM(older: BakeSizes, newer: BakeSizes): number {
  let changeM = 0;
  for (let n = 0; n < newer.coarseM.length; n += 1) {
    changeM = Math.max(changeM, Math.abs((newer.coarseM[n] ?? 0) - (older.coarseM[n] ?? 0)));
  }
  return changeM;
}

/** A drafted bake waiting for its refined normals. */
interface Draft {
  /** Its vertices' packed normals, {@link draftVertexBits}' of its tile. */
  readonly vertexBits: Uint16Array;
  /** The refinement's {@link RefineRequest.ticket}. */
  readonly ticket: number;
}

/** A resident patch's place for the graticule at one step (R10.T11), kept until the step changes. */
interface HeldGraticule {
  readonly stepRad: number;
  readonly patch: GraticulePatch;
}

/** A block's rebakes still to land, and whom to tell once they have (decision D7). */
interface Gate {
  readonly keys: Set<string>;
  readonly settled: () => void;
}

/** The state made per device: resources, cache and pool. */
interface Device {
  readonly resources: TerrainResources;
  readonly cache: PatchCache;
  /** Resolves the cache's draw set in place, allocating nothing after warm-up (one per cache). */
  readonly resolver: DrawSetResolver;
  /**
   * The draw set's morph holds at its coarse–fine edges and corners, by drawn index, made with each
   * selection's draw set (R10.T8.c).
   */
  readonly holds: CoarseFineHolds;
  /**
   * The drawn ground's transitions (R10.T16.i), which present each resolved draw set with its
   * coarsens' finer patches held, and the instances' fades they give a frame.
   */
  readonly morph: TimeMorph;
  readonly fades: TransitionFades;
  /**
   * Each slot's uploads on the device (plan R14, R14.T6.a): a caster's heights, survey mask and slot
   * record are its slot's last upload's, so that a cascade reused names each slot's version.
   */
  readonly slotVersions: Uint32Array;
  /** The draw time {@link Device.fades} were last made at, ms; `null` before any. */
  fadesAtMs: number | null;
  /**
   * Whether the device has drawn its ground yet (R10.T16.i): until a draw set leaves no seen
   * selected patch without a resident patch to draw it, it draws none (`#present`).
   */
  firstDrawn: boolean;
  /**
   * The drawn patches' areas the holds are made from, a sub-patch's its missing patch's, and the
   * levels they are drawn at, a sub-patch's its patch's, reused from selection to selection.
   */
  readonly drawnKeys: PatchKey[];
  readonly drawnLevels: number[];
  readonly pool: TerrainPool;
  readonly offBaked: () => void;
  readonly offFailed: () => void;
  readonly offRefined: () => void;
  /**
   * The resident patches whose bakes are drafts (R10.T6.a's progressive bake), by key string: each
   * one's vertices' packed normals ({@link draftVertexBits}) and the ticket of the refinement
   * asked, until its refined normals land.
   */
  readonly drafts: Map<string, Draft>;
  /** A lit terrain's tables of no air, for a body without an atmosphere; `null` for the spike's. */
  readonly airless: SurfaceLightingTables | null;
  material: MaterialHandle | null;
  /**
   * A lit terrain's depth material for the symbology's pass (R10.T16.c), compiled for the canvas;
   * `null` before, and for the spike's.
   */
  depthMaterial: MaterialHandle | null;
  /**
   * A lit terrain's caster into the shadow cascades on the high setting (R10.T9.b), compiled for
   * `depth-only`; `null` before, on the low setting, and for the other shadings.
   */
  shadowMaterial: MaterialHandle | null;
  /**
   * A lit terrain's emissive form (R08.T18.a), compiled at the first frame whose lighting carries
   * the ground's emission, which the plain form draws until it is made; `null` before.
   */
  emissiveMaterial: MaterialHandle | null;
  /** Whether the emissive form has been asked for on this device. */
  emissiveAsked: boolean;
  /**
   * The cluster path's on the high setting (plan R14, R14.T1.b): the interior patches' caster into
   * the cascades and the cull's kernel; `null` before, elsewhere, and where either cannot be made,
   * when the interior patches cast by the lit mesh.
   */
  clusterShadowMaterial: MaterialHandle | null;
  /** The interior patches' depth for the overlay on the cluster path (plan R14, R14.T1.e). */
  clusterDepthMaterial: MaterialHandle | null;
  cullKernel: ComputeHandle | null;
  /**
   * A lit terrain's interior patches' material (plan R14, R14.T1.a), the lit material without its
   * survey discard, and its emissive form, asked for with the emissive form; `null` before, and
   * for the other shadings, when the interior patches are drawn with the others.
   */
  interiorMaterial: MaterialHandle | null;
  emissiveInteriorMaterial: MaterialHandle | null;
  /**
   * The interior patches' lit form on the cluster path and its emissive form (plan R14, R14.T2.a),
   * asked for as the interior forms are, and the stable compaction's kernel; `null` before, for the
   * other shadings, and where one cannot be made, when the interior patches draw by the interior
   * mesh.
   */
  litClusterMaterial: MaterialHandle | null;
  emissiveLitClusterMaterial: MaterialHandle | null;
  compactKernel: ComputeHandle | null;
  /**
   * The edge patches' may-discard clusters' forms on the cluster path (plan R14, R14.T1.g): the lit
   * form and its emissive form, asked for as the lit cluster forms are, their depth for the overlay,
   * and on the high setting their caster; `null` before, for the other shadings, and where one
   * cannot be made, when the edge patches draw by the lit mesh.
   */
  litFinMaterial: MaterialHandle | null;
  emissiveLitFinMaterial: MaterialHandle | null;
  finDepthMaterial: MaterialHandle | null;
  finShadowMaterial: MaterialHandle | null;
  /**
   * The wireframe's interior patches' form on the cluster path (plan R14, R14.T4.c), made with the
   * cull's and the compaction's kernels; `null` before, for the other shadings, and where one cannot
   * be made, when the wireframe draws every patch by the lit mesh.
   */
  wireClusterMaterial: MaterialHandle | null;
  /** Each resident patch's layers' summary and request, by key string. */
  readonly residents: Map<string, Resident>;
  /** The resident patches asked for again by the rebake rule, at their last requests. */
  readonly rebakes: Map<string, PatchRequest>;
  /** The blocks whose rebakes are still to land, oldest first. */
  readonly gates: Gate[];
  /** The last demand's requests, by key string, for the priority of a bake as it lands. */
  readonly asked: Map<string, PatchRequest>;
  /**
   * The resident patches whose rebake the pool gave up (its `onFailed`): the lane asks no more for
   * them, so no later block's gate waits on them (the batch review's S7); a new bake of one, or its
   * eviction, takes it off.
   */
  readonly givenUp: Set<string>;
  /** Each resident patch's place for the wireframe's graticule, by key string (R10.T11). */
  readonly graticules: Map<string, HeldGraticule>;
}

/** One view's own frame state: its records, bands, uniforms, draw and debounce. */
interface ViewState {
  readonly id: TerrainViewId;
  readonly debounce: TerrainAnnunciationDebounce;
  readonly instances: InstanceRecords;
  readonly contacts: ContactRecords;
  /**
   * Each level's morph band, start then end, at the view's slewed tolerance
   * ({@link ViewState.bandTauPx}).
   */
  readonly morph: Float64Array;
  /**
   * The tolerance the bands are at, pixels: the last selection's {@link effectiveTauPx}, reached at
   * most {@link BAND_SLEW_PER_STEP} a step ({@link slewedTauPx}; plan R10, R10.T16.i); `null` before
   * the view's first selection.
   */
  bandTauPx: number | null;
  /** The last selection's {@link effectiveTauPx} for the view, pixels, which the bands follow. */
  targetTauPx: number | null;
  readonly uniforms: {
    readonly bodyRotation: Float32Array;
    readonly sunDirection: Float32Array;
    readonly sunRadiance: Float32Array;
    readonly atlas: Float32Array;
  };
  /**
   * The lit material's `Draw` (`terrainLit.wgsl`), rewritten in place each frame: its own fields,
   * then the shadow cascades' (R10.T9.b; `packCascadeUniforms`), w = 0 where none are drawn. The
   * cascades' are typed as a mapped type, not by `CascadeUniforms`' interface, which has no index
   * signature, so that the whole is a `DrawItem`'s uniforms.
   */
  readonly litUniforms: {
    readonly bodyRotation: Float32Array;
    readonly cameraBodyFixedM: Float32Array;
    readonly atlas: Float32Array;
  } & Readonly<Record<keyof CascadeUniforms, Float32Array>>;
  /**
   * The casters' `Draw` (`terrainShadow.wgsl`, R10.T9.b): the lit `Draw`'s own three fields, the
   * same arrays, so that a caster places its vertices as the lit draw does.
   */
  readonly casterUniforms: {
    readonly bodyRotation: Float32Array;
    readonly cameraBodyFixedM: Float32Array;
    readonly atlas: Float32Array;
  };
  /**
   * The four cascades' snapped centres from the last frame its pass fitted them in, body-fixed, m,
   * handed back to the next fit (decision-r10-t9-split, ruling 7 amended); `null` in a view new to
   * the pass, after a rebuild, and after a frame in which this pass did not take the view's
   * cascades (decision-r10-t9b-design D6(d)).
   */
  centres: readonly [Vec3, Vec3, Vec3, Vec3] | null;
  /**
   * Its far cascades' holds (plan R14, R14.T6.b), cut wherever its centres are dropped, at a change
   * of the geometry settings and after a frame that renders no cascades; and this frame's plan of
   * them, `null` in a frame without cascades.
   */
  readonly farHolds: FarCascadeHolds;
  farPlan: FarCascadePlan | null;
  /** Its cascades' caster inputs as each was last rendered (plan R14, R14.T6.a). */
  readonly casterInputs: CascadeReuse;
  /** The draw set's index of each of this frame's instance records, in their order (R10.T9.b). */
  readonly drawn: number[];
  /** Its last frame's patches and casters, for its counts (plan R14; {@link TerrainPass.counts}). */
  lastPatches: TerrainDrawnPatches;
  lastCasters: CasterFrame | null;
  /**
   * The cluster pass its last frame's overlay took its depth from (plan R14, R14.T1.e; T2.a): its
   * own, the lit pass's by the redraw, or `null` for the redraw of its draws.
   */
  lastOverlayPass: number | null;
  /**
   * Whether its last frame drew its interior patches through the lit pass's camera pass on the
   * cluster path (plan R14, R14.T2.a), the wireframe's alike (R14.T4.c), whose counts then hold its
   * clusters' triangles.
   */
  lastLit: boolean;
  /** Its cluster path's state (plan R14), made at its first frame on it. */
  clusters: ViewClusters | null;
  /**
   * The interior patches' records this frame (plan R14, R14.T1.a), apart from {@link
   * ViewState.instances}, which then hold the other patches', and the draw set's index of each; with
   * them, in the draw set's order, the edge patches the cluster path draws (plan R14, R14.T1.g),
   * whose places in it {@link ViewState.interiorFins} marks.
   */
  readonly interior: InstanceRecords;
  readonly interiorDrawn: number[];
  readonly interiorFins: boolean[];
  /**
   * Of the interior records, whether each one's heights fade from another surface this frame (plan
   * R10, R10.T16.i: a rebake's older bake, a rise's datum, a refine's or a coarsen's parent), which
   * its slot's cluster records do not bound, so that the cull keeps it whole (`CULL_INPUT_WHOLE`).
   */
  readonly interiorFading: boolean[];
  /** The interior patches' draw, and the view's draw it was made beside; `null` before. */
  interiorDraw: DrawItem | null;
  interiorBeside: DrawItem | null;
  /** The cascades' caster records and draws, made at the view's first caster frame (R10.T9.b). */
  casters: ViewCasters | null;
  /** The shadow texture the lit draw binds: the set's layers or the placeholder (R10.T9.b). */
  drawnShadow: TextureHandle | null;
  /** The wireframe's `Draw` (`terrainWire.wgsl`), rewritten in place each frame (R10.T11). */
  readonly wireUniforms: {
    readonly bodyRotation: Float32Array;
    /** Zeros: `terrainPatch.wgsl` names it, and the wireframe reads no normals. */
    readonly atlas: Float32Array;
    readonly contour: Float32Array;
    readonly contourWidthsPx: Float32Array;
    readonly graticule: Float32Array;
    readonly muted: Float32Array;
    readonly text: Float32Array;
    readonly casing: Float32Array;
    readonly occluderSlopePx: Float32Array;
  };
  /** The wireframe's phase records, one a drawn patch; `null` for the other shadings. */
  readonly phases: WirePhaseRecords | null;
  /**
   * The wireframe's interior patches' phase records on the cluster path (plan R14, R14.T4.c), one an
   * interior record in its order, {@link ViewState.phases} then the others'; `null` for the other
   * shadings.
   */
  readonly interiorPhases: WirePhaseRecords | null;
  /** Its draw, made once the material is ready on the device it was made for. */
  draw: DrawItem | null;
  drawDevice: Device | null;
  /** The phase table and atmosphere tables a lit draw binds, to make it again when they change. */
  drawnTable: TextureHandle | null;
  drawnAtmosphere: SurfaceLightingTables | null;
}

/** One view's casters into the four cascades (R10.T9.b; decision-r10-t9b-design D4). */
interface ViewCasters {
  /** Each cascade's records this frame, a byte-identical subset of the view's. */
  readonly records: readonly [InstanceRecords, InstanceRecords, InstanceRecords, InstanceRecords];
  /** Each cascade's caster draw but its count, on {@link ViewCasters.device}; `null` before. */
  items: readonly [DrawItem, DrawItem, DrawItem, DrawItem] | null;
  /** The device and material the draws were made for, to make them again when either changes. */
  device: Device | null;
  material: MaterialHandle | null;
}

/**
 * A lit view's cluster path (plan R14, R14.T1.b and T1.e): each interior record's cull input (its
 * passes' mask and its error unit's `f32` bits), the passes and the parameters as the cull reads
 * them, the bucket draws' shared parts of the casters and of the overlay's depth on
 * {@link ViewClusters.device}, and the counters, made when the view's measurement is on.
 */
interface ViewClusters {
  readonly inputs: Uint32Array;
  readonly passes: ArrayBuffer;
  readonly params: ArrayBuffer;
  casterBase: BucketDrawBase | null;
  overlayBase: BucketDrawBase | null;
  /**
   * The lit pass's bucket draws' shared part (plan R14, R14.T2.a): the interior draw's but its mesh,
   * indirect arguments and material, with the lists, and the interior draw and the form it was made
   * from; `null` before. On a wireframe pass the same camera pass's (`CLUSTER_LIT_PASS`), the
   * wireframe draw's (R14.T4.c).
   */
  litBase: BucketDrawBase | null;
  litBeside: DrawItem | null;
  /**
   * The may-discard buckets' draws' shared parts (plan R14, R14.T1.g): the lit pass's, the lit
   * base with the fin form, the casters' and the overlay's depth's; `null` before.
   */
  finLitBase: BucketDrawBase | null;
  finCasterBase: BucketDrawBase | null;
  finDepthBase: BucketDrawBase | null;
  device: Device | null;
  stats: ClusterStats | null;
}

/** No caster passes: a frame that draws no cascades. */
const NO_PASSES: ReadonlyArray<DepthLayerPass> = [];

/** No draws: a frame that draws no terrain. */
const NO_DRAWS: ReadonlyArray<DrawItem> = [];

/**
 * A view-frame's cluster path (plan R14, R14.T1.b and T1.e): its buckets, its cull inputs' buffer
 * and their staging, the cluster mesh, the bucket draws' shared parts of the casters (`null` before
 * the cluster caster is made) and of the overlay's depth (`null` where the overlay does not take
 * it from the cluster path), whether it does, and the cull's kernel.
 */
interface ClusterPath {
  readonly buckets: ClusterBuckets;
  readonly cullInputs: BufferHandle;
  readonly inputs: Uint32Array;
  readonly mesh: MeshHandle;
  readonly casterBase: BucketDrawBase | null;
  readonly overlayBase: BucketDrawBase | null;
  /** The interior patches' cluster depth's shared part, whatever the overlay takes; `null` before. */
  readonly depthBase: BucketDrawBase | null;
  readonly overlay: boolean;
  readonly kernel: ComputeHandle;
  /**
   * The lit pass's bucket draws' shared part where the interior patches draw through it (plan R14,
   * R14.T2.a), else `null`; its marks and tallies, and the stable compaction's kernel.
   */
  readonly litBase: BucketDrawBase | null;
  readonly marks: BufferHandle;
  readonly tallies: BufferHandle;
  readonly compact: ComputeHandle | null;
  /**
   * The may-discard buckets' draws' shared parts where the edge patches draw through the path (plan
   * R14, R14.T1.g): the lit pass's, the casters' and the overlay's depth's; `null` where none does.
   */
  readonly finLitBase: BucketDrawBase | null;
  readonly finCasterBase: BucketDrawBase | null;
  readonly finDepthBase: BucketDrawBase | null;
}

/** The overlay's pass of the cull (plan R14, R14.T1.e), after the four cascades', and its bit. */
const OVERLAY_PASS_BIT = 1 << CLUSTER_OVERLAY_PASS;

/** The lit pass's pass of the cull (plan R14, R14.T2.a), after the overlay's, and its bit. */
const LIT_PASS_BIT = 1 << CLUSTER_LIT_PASS;

/** The cull's passes' names for the counters: the cascades', the overlay's depth and the lit pass. */
const CLUSTER_PASS_NAMES: ReadonlyArray<string> = [...CASCADE_PASS_NAMES, "overlay depth", "lit"];

/**
 * How far each side of the overlay's frustum is opened for its cull, rad: past the `f32` rounding
 * of the planes and the scene's and the canvas's aspects, so that no cluster that draws is refused.
 */
const OVERLAY_CULL_MARGIN_RAD = 0.01;

/** A frame's caster passes and their counts, as {@link TerrainPass}'s `#drawCasters` drew them. */
type CasterFrame = Pick<TerrainCascadeFrame, "passes" | "litMeshCasters" | "clustered" | "reused">;

/** An instance record's 32-bit words. */
const INSTANCE_WORDS = INSTANCE_RECORD_BYTES / 4;

/** The overlay's depths, by their word in the cascades' caster inputs (plan R14, R14.T6.a). */
const OVERLAY_DEPTH_WORD = {
  redraw: 0,
  "scene-depth": 1,
  cluster: 2,
} as const satisfies Record<GeometrySettings["overlayDepth"], number>;

/**
 * Appends instance record `record` of `words` to `reuse`, as a cascade's caster input (plan R14,
 * R14.T6.a): its words as written, and the upload version, by `slotVersions`, of each slot its
 * caster reads: its own, and while its heights mix ({@link fadeWord}) the older bake's or, for a
 * refine's or a coarsen's ({@link PARENT_MIX_SLOT}), its parent's.
 */
export function appendCasterRecord(
  reuse: CascadeReuse,
  words: Uint32Array,
  record: number,
  slotVersions: Uint32Array,
): void {
  const base = record * INSTANCE_WORDS;
  reuse.words(words, base, base + INSTANCE_WORDS);
  reuse.word(slotVersions[words[base + INSTANCE_SLOT_WORD] ?? 0] ?? 0);
  const fades = words[base + INSTANCE_FADES_WORD] ?? 0;
  if (((fades >>> 8) & MIX_STEPS) === 0) {
    return;
  }
  const older = fades >>> 20;
  if (older === PARENT_MIX_SLOT) {
    const parent = words[base + INSTANCE_PARENT_SLOT_WORD] ?? NO_PARENT_SLOT;
    if (parent !== NO_PARENT_SLOT) {
      reuse.word(slotVersions[parent] ?? 0);
    }
  } else if (older !== DATUM_SLOT) {
    reuse.word(slotVersions[older] ?? 0);
  }
}

/** No cascade reused (plan R14, R14.T6.a). */
const NONE_REUSED: readonly [boolean, boolean, boolean, boolean] = [false, false, false, false];

/** No casters: a frame that draws no cascades. */
const NO_CASTERS: CasterFrame = {
  passes: NO_PASSES,
  litMeshCasters: [0, 0, 0, 0],
  clustered: false,
  reused: NONE_REUSED,
};

/**
 * The lit materials a frame draws with (R08.T18.a; plan R14, R14.T1.a): the other patches' form
 * and the interior patches' form of the same, each `null` where it is not made.
 */
interface LitForms {
  readonly edge: MaterialHandle | null;
  readonly interior: MaterialHandle | null;
  /** The interior patches' form on the cluster path (plan R14, R14.T2.a), or `null`. */
  readonly cluster: MaterialHandle | null;
  /** The edge patches' may-discard clusters' form on it (plan R14, R14.T1.g), or `null`. */
  readonly fin: MaterialHandle | null;
}

/** No lit forms: the other shadings' frames. */
const NO_FORMS: LitForms = { edge: null, interior: null, cluster: null, fin: null };

/** The cascades' indices, nearest first. */
const CASCADES: readonly [CascadeIndex, CascadeIndex, CascadeIndex, CascadeIndex] = [0, 1, 2, 3];

/** Every cascade rendered, as a frame without a plan of its far cascades renders them. */
const ALL_CASCADES_RENDERED: readonly [boolean, boolean, boolean, boolean] = [
  true,
  true,
  true,
  true,
];

/**
 * The horizon map's texel at the camera's nadir, m, as the cascades' weight compares cascade 1's
 * against it (plan R10, R10.T9.b; decision-r10-t9b-design D6(a)): the vertex spacing of the level L
 * CDLOD draws at distance h in the view, × 2^k, k the level's morph factor at h.
 *
 * @remarks
 * L is the level selection's traversal reaches at h: from level 0 down, to each next level while h
 * lies inside that level's band end in `morph`, so that h falls in [end₍L+1₎, end_L) where the
 * ends fall with level, and the coarsest, level 0, beyond them all; k = clamp((h − start_L) ÷
 * (end_L − start_L), 0, 1), 0 at level 0, which morphs to nothing. The spacing is `vertexSpacing`'s
 * mean at the equatorial radius. Since a level's spacing is twice its child's, the texel is
 * continuous in h through each split as through each band, and is defined at every height, with no
 * drawn patch to look up: a lookup would step w whenever the nadir's patch changes level, stands
 * in or leaves the view's records (decision-r10-t9-split §5(d)).
 *
 * The walk descends from the root, as selection does, because the band ends need not fall with
 * level: a level's selection error, its bound plus its chord's sagitta, has bumps at fine levels.
 * Walked up from the finest level instead, a fine level's bump stopped it too fine: on `TERRAIN
 * TEST` at h = 6.6 km it gave 0.605 m, 19.35 m ÷ 32, five levels too fine, in 574 of 900 frames
 * over 190–205 s (the RTX 3080, hidden), so that w fell to 0 and the cascades switched on and off.
 *
 * @param morph - A view's morph bands, start then end a level (`ViewState.morph`, at its
 *   effective τ).
 * @param heightAboveGroundM - h, the camera's height above the drawn ground at its nadir, m; below
 *   0 it is taken as 0.
 */
export function cascadeHorizonTexelM(
  planet: PlanetGeometry,
  morph: Float64Array,
  heightAboveGroundM: number,
): number {
  const h = Math.max(heightAboveGroundM, 0);
  let level = 0;
  while (level < planet.finestLevel && h < (morph[2 * (level + 1) + 1] ?? 0)) {
    level += 1;
  }
  let k = 0;
  if (level > 0) {
    const start = morph[2 * level] ?? 0;
    const end = morph[2 * level + 1] ?? 0;
    k = end > start ? Math.min(1, Math.max(0, (h - start) / (end - start))) : 0;
  }
  return vertexSpacing(planet.figure.equatorialRadiusM, level).meanM * 2 ** k;
}

/**
 * A spheroid's mean radius, m: (2a + c) ÷ 3, the arithmetic mean of its three semi-axes (IUGG's
 * R₁, Moritz 1980, "Geodetic Reference System 1980", Bulletin Géodésique 54), the R the cascades'
 * caster reach takes (R10.T9.a's `casterReachM`).
 */
export function meanRadiusM(
  figure: Pick<PlanetGeometry["figure"], "equatorialRadiusM" | "polarRadiusM">,
): number {
  return (2 * figure.equatorialRadiusM + figure.polarRadiusM) / 3;
}

/**
 * Whether a patch casts into cascade `index` of `set` (plan R10, R10.T9.b;
 * decision-r10-t9b-design D4): whether its bounding sphere meets the cascade's light-space box,
 * the sphere's square on the light's axes, from the far plane, r behind the snapped centre, to the
 * caster reach towards the sun beyond the sphere.
 *
 * @remarks
 * With v the patch's centre less the cascade's snapped centre in the light's axes (x_L, y_L, and
 * z_L towards the sun), r the cascade's radius, ρ the patch's and ℓ the set's caster reach: kept
 * where |v·x_L| ≤ r + ρ, |v·y_L| ≤ r + ρ and −r − ρ ≤ v·z_L ≤ r + ℓ + ρ. On both capability paths
 * a caster up to ℓ nearer the sun than the sphere shadows it: pancaked onto the near plane by
 * `unclippedDepth`, or inside the pulled-back near plane.
 *
 * @param fromCameraBodyFixedM - The patch's centre less the camera, body-fixed, m (`f64`).
 * @param radiusM - ρ, the patch's bounding radius about that centre, m.
 */
export function castsInto(
  set: CascadeSet,
  index: CascadeIndex,
  fromCameraBodyFixedM: Vec3,
  radiusM: number,
): boolean {
  const { x: xL, y: yL, z: zL } = set.basis;
  const p = fromCameraBodyFixedM;
  const cascade = set.cascades[index];
  const o = cascade.centreFromCameraLightM;
  const reachM = cascade.radiusM + radiusM;
  const vx = xL.x * p.x + xL.y * p.y + xL.z * p.z - o.x;
  const vy = yL.x * p.x + yL.y * p.y + yL.z * p.z - o.y;
  const vz = zL.x * p.x + zL.y * p.y + zL.z * p.z - o.z;
  return (
    Math.abs(vx) <= reachM &&
    Math.abs(vy) <= reachM &&
    vz >= -reachM &&
    vz <= reachM + set.casterReachM
  );
}

/** A view as selection takes it, with the pass's name for it and the τ of its morph bands. */
interface PassSelectionView extends ViewSelectionInput {
  readonly id: TerrainViewId;
  /** The view's τ, pixels, at which its morph bands are made: selection's `tauPx` is τ ÷ (1 + m). */
  readonly bandTauPx: number;
}

/** One view as the last selection took it. */
interface SelectedView {
  readonly id: TerrainViewId;
  readonly cameraBodyFixedM: Vec3;
  readonly orientationBodyFixed: Quaternion;
  readonly fovXRad: number;
  readonly viewport: ViewSize;
  readonly tauPx: number;
  readonly weight: number;
  /**
   * The distance to the box of the nearest selected patch it sees not at the finest level, metres,
   * at least one finest patch: a box with a wide height range can contain the camera; `+∞` with
   * none.
   */
  readonly nearestM: number;
}

/** The inputs of the last selection, to decide whether to select again, and what it gave. */
interface Selected {
  readonly selection: Selection;
  /**
   * The demand the pass asks for: the selection's, joined by its lookahead's where it looked ahead
   * (`priority.ts`' `joinDemand`), the cache's demand pins with it.
   */
  readonly demand: ReadonlyArray<PatchRequest>;
  /** The draw set as the time morph presents it (R10.T16.i), which every draw reads. */
  readonly drawSet: DrawSet;
  /** The resolver's draw set for the selection, which a coarsen's end presents again. */
  readonly resolved: DrawSet;
  /** The contacts that reach a drawn patch, body-fixed. */
  readonly nearContacts: ReadonlyArray<GroundContact>;
  /** The views, in the selection's order: view k is bit k of the masks. */
  readonly views: ReadonlyArray<SelectedView>;
  /** A copy of the contacts given, so that a caller's array changed in place is still seen. */
  readonly grounded: ReadonlyArray<GroundContact>;
  readonly rangesVersion: number;
  readonly surveyVersion: number;
}

/** The body-fixed axes into the non-rotating ones as `mat4x4f`, column-major. */
function rotationMat4(rotation: Rotation3, out: Float32Array): void {
  const [r0, r1, r2] = rotation.rows;
  out.set([r0.x, r1.x, r2.x, 0, r0.y, r1.y, r2.y, 0, r0.z, r1.z, r2.z, 0, 0, 0, 0, 1]);
}

/** Appends `value` to `latest`, dropping the oldest beyond `most`. */
function pushLatest(latest: number[], value: number, most: number): void {
  latest.push(value);
  if (latest.length > most) {
    latest.shift();
  }
}

function distance(a: Vec3, b: Vec3): number {
  return Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z);
}

function sameContacts(a: ReadonlyArray<GroundContact>, b: ReadonlyArray<GroundContact>): boolean {
  return (
    a.length === b.length &&
    a.every((c, i) => {
      const d = b[i];
      return (
        d !== undefined &&
        c.radiusM === d.radiusM &&
        c.positionM.x === d.positionM.x &&
        c.positionM.y === d.positionM.y &&
        c.positionM.z === d.positionM.z
      );
    })
  );
}

/** Whether two level tables hold the same numbers, or are both absent. */
function sameLevels(a: Float64Array | null, b: Float64Array | null): boolean {
  if (a === null || b === null) {
    return a === b;
  }
  return a.length === b.length && a.every((value, n) => Object.is(value, b[n]));
}

/** The conditions of a frame with nothing to say: no stand-in, nothing missing, nothing limited. */
const CLEAR: TerrainConditions = { streaming: false, detailLimited: false };

/**
 * The CDLOD morph band of `level` in a view, metres: the morph begins at {@link
 * MORPH_START_FRACTION} of the way from the distance where the level meets the view's τ to the
 * distance where its parent does, and is complete there. Level 0 has no parent and no band; the
 * finest level, whose own error is zero, has [0.7 d₍ₙ₋₁₎, d₍ₙ₋₁₎].
 */
export function morphRangeM(
  planet: PlanetGeometry,
  level: number,
  view: ViewSelectionInput,
): readonly [number, number] {
  if (level === 0) {
    return [0, 0];
  }
  // Metres of distance per metre of error at which the error subtends τ (Design note 7).
  const distancePerErrorM = view.viewport.widthPx / (2 * Math.tan(view.fovXRad / 2) * view.tauPx);
  const own = selectionErrorM(planet, level) * distancePerErrorM;
  const parent = selectionErrorM(planet, level - 1) * distancePerErrorM;
  return [own + MORPH_START_FRACTION * (parent - own), parent];
}

/**
 * The tolerance a view's morph bands are computed at, pixels: τ × max(1, `limitExcess` ÷ w) for
 * the setting's τ and a view of weight w (decision-r05-high-bound.md, F2).
 *
 * @remarks
 * Selection runs at τ_sel = τ ÷ (1 + m), for m = `SELECTION_MARGIN`, and measures `limitExcess`
 * against it, so every drawn baked leaf has ρ ≤ τ′ = τ_sel × max(1, `limitExcess` ÷ w), the
 * effective tolerance of `Selection.limitExcess` and T13.a's record. The bands, at (1 + m) τ′,
 * keep the margin over the leaves that they have at τ without a budget. A coarse leaf then lies
 * beyond its finer neighbour's band, and stays beyond it between selections, whose moves of at
 * most {@link RESELECT_MOVE_FRACTION} × d_min use up exactly that margin. Their shared edge stays
 * at morph 1 under the budget too, and a split's children start fully morphed. `limitExcess` is 0
 * when the budget does not bind, which makes the result exactly τ and the bands bit-identical to
 * an unbudgeted selection's.
 *
 * @param weight - The view's streaming weight, greater than 0.
 */
export function effectiveTauPx(tauPx: number, limitExcess: number, weight: number): number {
  return tauPx * Math.max(1, limitExcess / weight);
}

/**
 * The budget's hold's margin (plan R10, R10.T16.i; {@link TerrainPassOptions.budgetHoldMargin}): the
 * share of the patch budget the last selection's splits alone may take, a hand value (lean,
 * 2026-11-02), against seamless-1's finding of patches coarsening and refining by turns at the
 * budget's margin: the seamless probe's flip-flops on high at 8 workers with drafts at half fell
 * from 3.6 a second at 5% to 0.6 at 15% and none at 30%.
 */
export const BUDGET_HOLD_MARGIN = 0.15;

/**
 * The most a view's morph bands' tolerance moves a step, as a fraction (plan R10, R10.T16.i; a
 * hand value): the budget's τ′ steps by up to 50 to 63 times between selections (TERRAIN TEST,
 * research-terrain-seamless), and a vertex in its band moves by about 6 Δτ′ ÷ τ′ in k, so that the
 * whole screen re-morphed at once.
 */
export const BAND_SLEW_PER_STEP = 0.02;

/**
 * The bands' tolerance a step on from `currentPx` towards `targetPx`, pixels: at most a factor of
 * 1 + {@link BAND_SLEW_PER_STEP} up or down; `targetPx` itself with no current one. Selection's τ
 * and `limitExcess` are not slewed: only {@link morphRangeM}'s input is.
 */
export function slewedTauPx(currentPx: number | null, targetPx: number): number {
  if (currentPx === null) {
    return targetPx;
  }
  const step = 1 + BAND_SLEW_PER_STEP;
  return Math.min(currentPx * step, Math.max(currentPx / step, targetPx));
}

/** A view's camera as a step took it, body-fixed: where it stood and how it looked. */
export interface SteppedCamera {
  /** Its position from the body's centre, body-fixed metres. */
  readonly positionM: Vec3;
  /** Its orientation in the body-fixed axes. */
  readonly orientation: Quaternion;
}

/**
 * The angle a view's frustum is opened by for its selection under `motion`
 * ({@link TerrainFrustumMargin}), rad (plan R10, R10.T16; the batch review's S6): one pixel's
 * angle, the turn the pass lets a view make before it selects again, plus the view's motion since
 * its previous step, the angle it turned and the angle its translation moved its nearest selected
 * ground by (|Δp| ÷ d), at most {@link MAX_FRUSTUM_MARGIN_RAD}.
 *
 * @remarks
 * Between selections a view turns by up to a pixel, and a view may draw a frame after the step that
 * selected for it (an instrument draws after the primary's step, so that its report reaches the
 * step after). A strip at its leading edge would then have no patch selected while its coverage
 * mask discards the reference surface there, and the sky would show through the body. Opened by
 * the reselection budget and one frame's motion, the selection holds that strip, and a patch of it
 * not yet baked withholds the view's mask as any missing patch does (decision D7).
 *
 * @param previous - The view's camera at its previous step, or `null` at its first.
 * @param nearestM - The distance to the view's nearest selected ground, metres (the move rule's
 *   d), or `+∞` where none is known, when its translation counts for nothing.
 */
export function frustumMarginRad(
  view: Pick<ViewSelectionInput, "camera" | "fovXRad" | "viewport">,
  previous: SteppedCamera | null,
  nearestM: number,
): number {
  const pixelRad = view.fovXRad / view.viewport.widthPx;
  if (previous === null) {
    return Math.min(pixelRad, MAX_FRUSTUM_MARGIN_RAD);
  }
  const q = view.camera.orientation;
  const p = previous.orientation;
  const dot = Math.abs(q.w * p.w + q.x * p.x + q.y * p.y + q.z * p.z);
  const turnedRad = 2 * Math.acos(Math.min(dot, 1));
  const movedRad =
    Number.isFinite(nearestM) && nearestM > 0
      ? distance(view.camera.positionM, previous.positionM) / nearestM
      : 0;
  return Math.min(pixelRad + turnedRad + movedRad, MAX_FRUSTUM_MARGIN_RAD);
}

/** The terrain pass of one body, shared by the views that hand it over. */
export class TerrainPass {
  readonly #engine: RenderEngine;
  readonly #setting: QualitySetting;
  /** The terrain's settings, resolved once: the setting's own, or the spike's variant. */
  readonly #terrain: TerrainSettings;
  readonly #source: HeightSourceSetting;
  readonly #createPool: TerrainPoolFactory;
  readonly #survey: TerrainSurveyLookup | null;
  readonly #measureSelection: boolean;
  readonly #onResident: ((key: PatchKey) => void) | null;
  /** The last draft's refinement ticket ({@link RefineRequest.ticket}), from 0. */
  #tickets = 0;
  readonly #onSelect: ((input: SelectionInput, selection: Selection) => void) | null;
  /** The observers ({@link TerrainPass.observe}), replaced whole on each change. */
  #observers = NO_OBSERVERS;
  readonly #frustumMargin: TerrainFrustumMargin;
  readonly #morphInMs: number | undefined;
  readonly #budgetHoldMargin: number | null;
  readonly #surveyedFloorLevel: number;
  /** Whether the drawn ground's changes cut ({@link TerrainPass.setReducedMotion}). */
  #reducedMotion = false;
  /** The pixels each change moves the ground by, from the resident patches' sizes (R10.T16.i). */
  readonly #morphSpans: MorphSpans;
  /** The lookahead's clock ({@link TerrainPassOptions.lookaheadClockMs}), or `null` for none. */
  readonly #lookaheadClockMs: (() => number) | null;
  /** The time of the last step on the lookahead's clock, ms; `null` before the first. */
  #steppedAtMs: number | null = null;
  /** When each key of the demand asked now was first asked, on the lookahead's clock, ms. */
  readonly #askedAtMs = new Map<string, number>();
  /** The latest {@link LOOKAHEAD_SAMPLES} demand → resident times, s, oldest first. */
  readonly #roundTripsS: number[] = [];
  /** The selection time of each of the latest {@link LOOKAHEAD_COST_STEPS} steps, ms, oldest first. */
  readonly #stepCostsMs: number[] = [];
  /** The selection time of the step under way, ms. */
  #stepCostMs = 0;
  /** Selections since the lookahead last selected, for its cadence while selection is slow. */
  #sinceAhead = LOOKAHEAD_SLOW_EVERY;
  /** The lookahead's last demand, or `null` where its views did not move. */
  #aheadDemand: ReadonlyArray<PatchRequest> | null = null;
  readonly #offRestored: () => void;
  readonly #shading: TerrainShading;
  /** The session's materials, which it draws with and never releases; `null` to compile its own. */
  readonly #materials: TerrainMaterialSource | null;
  /**
   * A lit pass's shadow cascades (R10.T9.b): the session's, or its own where none is given
   * ({@link TerrainPass}'s `#ownShadows`); `null` for the other shadings, which bind none.
   */
  readonly #shadows: TerrainShadows | null;
  /** Whether {@link TerrainPass}'s `#shadows` are the pass's own, to let go at its disposal. */
  readonly #ownShadows: boolean;
  /** The body's realised relief range, m, for the cascades' caster reach (R10.T9.b). */
  readonly #reliefM: () => number | null;
  /** How the pass draws its geometry (plan R14; {@link TerrainPass.setGeometry}). */
  #geometry: GeometrySettings;
  /** Whether each view's cluster counters are sampled ({@link TerrainPass.measureClusters}). */
  #measuring = false;
  /** The cluster cull's kernel ({@link TerrainPassOptions.clusterCullKernel}). */
  readonly #cullKernel: KernelPair;
  /** Why the cluster path was refused on the current device, `null` while it stands. */
  #clusterRefusal: string | null = null;
  /** Each view's own frame state, by its name. */
  readonly #views = new Map<TerrainViewId, ViewState>();
  /** Each view's camera at the last step, for its motion ({@link frustumMarginRad}). */
  readonly #stepped = new Map<TerrainViewId, SteppedCamera>();
  /**
   * Each view's nearest selected ground at the last selection that gave it one
   * ({@link SelectedView}'s `nearestM`, finite), for its motion margin after a selection that gave
   * none.
   */
  readonly #nearestGround = new Map<TerrainViewId, number>();
  /** What the render thread keeps of the resident bakes' heights (R10.T16.c), on the current device. */
  readonly #heights = new ResidentHeights();
  /** The planet as selection reads it: its figure and the field's level table, which grows. */
  #planet: PlanetGeometry;
  #rangesVersion = 0;
  #device: Device;
  #selected: Selected | null = null;
  #demandDirty = true;
  /**
   * Every height worker is lost, or the session halted the pass ({@link TerrainPass.halt}): it asks
   * for and draws nothing more.
   */
  #faulted = false;
  /** The cause its frames carry once faulted: the first one, which stands. */
  #cause: TerrainFault = TERRAIN_NOT_AVAILABLE;
  #disposed = false;

  /**
   * @throws RangeError for a body's source that skips its normals where the pass's slots hold them
   *   (`TERRAIN_SHADING_TRAITS`' `normals`): only the wireframe's bakes may skip them (R10.T11).
   */
  constructor(options: TerrainPassOptions) {
    this.#engine = options.engine;
    this.#setting = options.setting;
    this.#terrain = options.terrain ?? TERRAIN_SETTINGS[options.setting];
    this.#planet = options.planet;
    this.#source = options.source;
    this.#shading = terrainShadingOf(options.source);
    if (
      options.source.kind === "body" &&
      options.source.normals === "skipped" &&
      TERRAIN_SHADING_TRAITS[this.#shading].normals
    ) {
      throw new RangeError(`the ${this.#shading} terrain's slots hold the normals its bakes skip`);
    }
    this.#materials = this.#shading === "spike" ? null : (options.materials ?? null);
    this.#createPool = options.createPool;
    this.#survey = options.survey ?? null;
    this.#measureSelection = options.measureSelection ?? false;
    this.#onResident = options.onResident ?? null;
    this.#onSelect = options.onSelect ?? null;
    this.#frustumMargin = options.frustumMargin ?? "none";
    this.#morphInMs = options.morphInMs;
    this.#budgetHoldMargin =
      options.budgetHoldMargin === undefined ? BUDGET_HOLD_MARGIN : options.budgetHoldMargin;
    this.#surveyedFloorLevel = options.surveyedFloorLevel ?? 0;
    this.#morphSpans = {
      morphPx: (keyString) => this.#screenPx(keyString, this.#morphSizeM(keyString)),
      risePx: (keyString) =>
        this.#screenPx(keyString, this.#device.residents.get(keyString)?.sizes.reliefM ?? 0),
      changePx: (keyString, changeM) => this.#screenPx(keyString, changeM),
    };
    this.#lookaheadClockMs = options.lookaheadClockMs ?? null;
    this.#reliefM = options.reliefM ?? ((): number | null => null);
    this.#geometry = options.geometry ?? SETTINGS[options.setting].geometry;
    this.#cullKernel = options.clusterCullKernel ?? CLUSTER_CULL_KERNEL;
    if (this.#shading === "lit") {
      this.#ownShadows = options.shadows === undefined;
      this.#shadows =
        options.shadows ??
        new TerrainShadows(options.engine, { cascades: options.setting === "high" });
    } else {
      this.#ownShadows = false;
      this.#shadows = null;
    }
    let made = false;
    try {
      this.#device = this.#makeDevice();
      made = true;
    } finally {
      // A device not made (`EngineUnavailable` during a loss, or a disposed engine) leaves no pass
      // to dispose: its own cascades are let go here, so that their restore listener goes too.
      if (!made && this.#ownShadows) {
        this.#shadows?.dispose();
      }
    }
    this.#offRestored = this.#device.resources.onRebuilt(() => {
      this.#rebuild();
    });
  }

  /**
   * How the pass draws its geometry (plan R14): its options' `geometry`, its setting's
   * (`SETTINGS[setting].geometry`) unless given, until {@link TerrainPass.setGeometry}.
   */
  get geometry(): GeometrySettings {
    return this.#geometry;
  }

  /**
   * Switches how the pass draws its geometry from the next draw on (plan R14): with the cluster
   * path off (`GEOMETRY_BEFORE_R14`, the way back) or a lever off, it draws as it drew before
   * them, from the same selection, residents and records, so that a check draws one frame both
   * ways and holds them byte for byte, and a run measures both on one build.
   */
  setGeometry(geometry: GeometrySettings): void {
    this.#geometry = geometry;
    // A change of the settings is a cut of the far cascades' holds (plan R14, R14.T6.b).
    for (const state of this.#views.values()) {
      state.farHolds.cut();
    }
  }

  /**
   * Why the cluster path was refused on the current device (plan R14, Design note 16): the cull's
   * kernel or a cluster material its compiler refused, named with the refusal; `null` while the
   * path stands or before it is made. While refused, the interior patches cast by the lit mesh and
   * the overlay redraws its depth, today's path, from the next frame on; a restore makes it again.
   */
  get clusterRefusal(): string | null {
    return this.#clusterRefusal;
  }

  /**
   * The measurement switch of the cluster path's counters (plan R14, R14.T1.b): on, each frame
   * that casts through it samples its buckets' counts into a read-back ring
   * ({@link TerrainPass.clusterStats}); off, the default, an ordinary frame reads nothing back.
   */
  measureClusters(on: boolean): void {
    this.#measuring = on;
  }

  /**
   * View `id`'s cluster counters (plan R14, R14.T1.b): each cascade's clusters and triangles cast
   * through the cluster path, from the last sample to have come back ({@link CASCADE_PASS_NAMES}'
   * order); `null` before any, and while {@link TerrainPass.measureClusters} is off.
   */
  clusterStats(id: TerrainViewId): ReadonlyArray<ClusterPassStats> | null {
    return this.#measuring ? (this.#views.get(id)?.clusters?.stats?.latest() ?? null) : null;
  }

  /**
   * View `id`'s triangles at its last draw (plan R14, R14.T1.b; {@link TerrainGeometryCounts}), its
   * cluster path's from its counters ({@link TerrainPass.clusterStats}); `null` for a view the
   * pass holds no state of.
   */
  counts(id: TerrainViewId): TerrainGeometryCounts | null {
    const state = this.#views.get(id);
    if (state === undefined) {
      return null;
    }
    const { interior, edge, fins } = state.lastPatches;
    const edgeIndices = TERRAIN_SHADING_TRAITS[this.#shading].litMesh
      ? LIT_PATCH_INDICES
      : PATCH_INDICES;
    const cast = state.lastCasters;
    const stats = this.clusterStats(id);
    let casterTriangles: TerrainGeometryCounts["casterTriangles"] = null;
    if (cast !== null && (!cast.clustered || stats !== null)) {
      const [a, b, c, d] = cast.litMeshCasters.map(
        (n, k) => (n * LIT_PATCH_INDICES) / 3 + (cast.clustered ? (stats?.[k]?.triangles ?? 0) : 0),
      );
      casterTriangles = [a ?? 0, b ?? 0, c ?? 0, d ?? 0];
    }
    const drawnTriangles = (interior * LIT_INTERIOR_INDICES + edge * edgeIndices) / 3;
    // The edge patches the lit mesh draws: those on the cluster path are its counts' (R14.T1.g).
    const edgeTriangles = ((edge - fins) * edgeIndices) / 3;
    let overlayTriangles: number | null = drawnTriangles;
    if (state.lastOverlayPass !== null) {
      const overlayStats = stats?.[state.lastOverlayPass] ?? null;
      overlayTriangles = overlayStats === null ? null : edgeTriangles + overlayStats.triangles;
    }
    const litStats = state.lastLit ? (stats?.[CLUSTER_LIT_PASS] ?? null) : null;
    return {
      patches: state.lastPatches,
      drawnTriangles,
      litClusterTriangles: litStats === null ? null : edgeTriangles + litStats.triangles,
      casterTriangles,
      overlayTriangles,
    };
  }

  /** The current device's layout. */
  get layout(): TerrainLayout {
    return this.#device.resources.layout;
  }

  /** The selection budget: half the cache's slots (decision-r05-patch-demand.md, 4b). */
  get maxPatches(): number {
    return Math.floor(this.layout.slots.slotCount / 2);
  }

  /** How the pass shades its patches ({@link terrainShadingOf}). */
  get shading(): TerrainShading {
    return this.#shading;
  }

  /** The planet selection reads: the body's figure and its field's latest level table. */
  get planet(): PlanetGeometry {
    return this.#planet;
  }

  /** Whether every block's rebakes have landed: the body's mask may take every cell given. */
  get settled(): boolean {
    return this.#device.gates.length === 0;
  }

  /**
   * Whether every height worker is lost (R10.T16.b's fault), or the session halted the pass
   * (R10.T16.e's): nothing is asked or drawn now.
   */
  get faulted(): boolean {
    return this.#faulted;
  }

  /** The views of the pass's current draws, by name. */
  get viewIds(): ReadonlySet<TerrainViewId> {
    return new Set(this.#views.keys());
  }

  /**
   * The last step's selection and draw set, the draw set rewritten in place at the next selection,
   * or `null` before any selection and while faulted (R10.T16.c's survey edge and the tests read
   * them between steps).
   */
  /**
   * The drafted bakes' refinements asked and not yet landed (R10.T6.a's progressive bake): 0 once
   * every resident patch's normals are its whole bake's, as they are always where its pool drafts
   * none. A frame drawn before they land may shade otherwise than one drawn after.
   */
  get refining(): number {
    return this.#device.pool.refiningCount ?? 0;
  }

  /**
   * Whether the operator asks for reduced motion (plan R10, R10.T16.i; the UX guide's "Motion and
   * sound"): the drawn ground's changes then cut at their first draw rather than morph in, its
   * distance's morph, which moves with the camera alone, kept. Off as the pass is made.
   */
  setReducedMotion(on: boolean): void {
    this.#reducedMotion = on;
  }

  get lastStep(): Pick<TerrainStep, "selection" | "drawSet"> | null {
    const selected = this.#selected;
    return selected === null || this.#faulted
      ? null
      : { selection: selected.selection, drawSet: selected.drawSet };
  }

  /**
   * The lookahead's L, s (plan R10, R10.T14.b; `lookahead.ts`' `lookaheadSeconds`): the median of
   * the pass's latest {@link LOOKAHEAD_SAMPLES} demand → resident times, each from the step that
   * first asked for a patch to its bake's landing, within [0.25, 2] s; 0.25 s before any has landed,
   * and always for a pass with no lookahead clock, which times none.
   */
  get lookaheadS(): number {
    return lookaheadSeconds(this.#roundTripsS);
  }

  /**
   * Listens to the pass's selections, to the patches it makes resident and to the bakes its cache
   * refuses, from the next of each on, called where its options' `onSelect` and `onResident` are
   * (plan R10, R10.T14.c: the descent spike's metrics over a terrain session's pass, which the
   * session makes without them; the seamless probe's).
   *
   * @remarks
   * A pass with no observer calls none: its loops over them are empty.
   *
   * @returns The observer's removal, which takes effect at once.
   */
  observe(observer: TerrainPassObserver): () => void {
    this.#observers = [...this.#observers, observer];
    return () => {
      this.#observers = this.#observers.filter((each) => each !== observer);
    };
  }

  /**
   * The cache's resident patch keyed by `keyString` (a `patchKeyString`), or `undefined`: R10.T16.c
   * reads the resident bakes under the survey's edge.
   */
  resident(keyString: string): ReturnType<PatchCache["get"]> {
    return this.#device.cache.get(keyString);
  }

  /**
   * The resident bakes' heights as the survey edge's rim and its note's line of sight read them
   * (R10.T16.c; `residentHeights.ts`): a body's bakes, on the current device, forgotten as they
   * leave the cache and at a restore.
   */
  get heights(): ResidentHeightsView {
    return this.#heights;
  }

  /**
   * A lit pass's terrain as depth alone, compiled for the canvas (`terrainDepthMaterialSpec`,
   * R10.T16.c): a view's draw with this material writes the drawn ground's depth, pushed by its
   * constant fraction, into the symbology's pass before its lines; `null` before it is compiled, and for R05's
   * spike shading, which no overlay draws over.
   */
  get symbologyDepthMaterial(): MaterialHandle | null {
    return this.#device.depthMaterial;
  }

  /**
   * Compiles the material ahead of the first frame, and beside it a lit pass's depth material for
   * the symbology's pass, or takes the session's, compiled when the session was made
   * ({@link TerrainPassOptions.materials}; R10.T14.d); frames draw nothing until the first
   * resolves, and give no depth until the second does. A depth material that cannot be made is
   * reported, and the pass draws on without it. On the high setting a lit pass also compiles, or
   * takes, its caster into the shadow cascades (R10.T9.b); until it is made, and where it cannot
   * be, the view's draws take w = 0.
   */
  async ready(): Promise<void> {
    const device = this.#device;
    const path = device.resources.layout.vertexPath;
    const lit = this.#shading === "lit";
    const cascades = lit && this.#shadows?.cascades === true;
    const depth = Promise.all([
      lit ? this.#readyDepth(device, path) : Promise.resolve(),
      cascades ? this.#readyShadow(device, path) : Promise.resolve(),
      lit ? this.#readyInterior(device, "lit-interior", path) : Promise.resolve(),
      lit ? this.#readyClusters(device, path) : Promise.resolve(),
      cascades ? this.#readyClusterCasters(device, path) : Promise.resolve(),
      lit ? this.#readyLitClusters(device, "lit-cluster", path) : Promise.resolve(),
      lit ? this.#readyFin(device, "lit-fin", path) : Promise.resolve(),
      lit ? this.#readyFin(device, "depth-fin", path) : Promise.resolve(),
      cascades ? this.#readyFin(device, "shadow-fin", path) : Promise.resolve(),
      this.#shading === "wire" ? this.#readyWireClusters(device, path) : Promise.resolve(),
    ]);
    let material: MaterialHandle;
    try {
      material = await this.#materialOf(this.#shading, device, path);
    } catch (error: unknown) {
      // Refused for a device a restore replaced, during a loss (whose restore compiles it again),
      // or for a pass let go meanwhile: not a failure of the material (the batch review's C9).
      if (this.#superseded(device) || error instanceof EngineUnavailable) {
        await depth;
        return;
      }
      throw error;
    }
    if (device === this.#device && !this.#disposed && device.material === null) {
      device.material = material;
    } else {
      // Made for a device a restore replaced, a pass let go meanwhile, or a second call: no frame
      // ever drew it.
      this.#releaseOwn(material);
    }
    await depth;
  }

  /**
   * `kind`'s material on `path` for `device`: the session's where it gives its materials (a lit
   * pass's material and depth material, and the wireframe's), else compiled for the pass's own mesh
   * as `terrainMaterialOf` says (R10's lit material into the scene target, its depth material and
   * the wireframe's into the canvas), and R05's spike shading into the scene target (`rgba16float`).
   */
  #materialOf(
    kind:
      | TerrainShading
      | "depth"
      | "depth-cluster"
      | "shadow"
      | "shadow-cluster"
      | "lit-emissive"
      | "lit-interior"
      | "lit-emissive-interior"
      | "lit-cluster"
      | "lit-emissive-cluster"
      | FinKind
      | "wire-cluster",
    device: Device,
    path: TerrainLayout["vertexPath"],
  ): Promise<MaterialHandle> {
    if (kind === "spike") {
      return this.#engine.createMaterialAsync(
        terrainMaterialSpec(path),
        ["rgba16float"],
        [device.resources.mesh],
      );
    }
    if (this.#materials !== null) {
      return this.#materials.material(kind, path);
    }
    // The device's capabilities at each making: a caster made again after a restore onto another
    // adapter takes that adapter's `depth-clip-control` (decision-r10-t9b-design D1).
    const { spec, targets } = terrainMaterialOf(kind, path, this.#engine.capabilities);
    return this.#engine.createMaterialAsync(spec, targets, [device.resources.mesh]);
  }

  /** Releases a material the pass compiled itself; the session's are the session's to release. */
  #releaseOwn(material: MaterialHandle): void {
    if (this.#materials === null) {
      this.#engine.releaseMaterial(material);
    }
  }

  /** Compiles, or takes, `device`'s depth material for the canvas (R10.T16.c); never rejects. */
  async #readyDepth(device: Device, path: TerrainLayout["vertexPath"]): Promise<void> {
    try {
      const depth = await this.#materialOf("depth", device, path);
      if (device === this.#device && !this.#disposed && device.depthMaterial === null) {
        device.depthMaterial = depth;
      } else {
        this.#releaseOwn(depth);
      }
    } catch (error: unknown) {
      // A device replaced, a loss or a pass let go meanwhile is no failure (C9); otherwise the
      // symbology has no terrain depth over the image, and the terrain itself still draws.
      if (!this.#superseded(device) && !(error instanceof EngineUnavailable)) {
        console.error("the terrain's depth material for the symbology failed:", error);
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s interior patches' lit material or its emissive form (plan R14,
   * R14.T1.a); never rejects. One that cannot be made is reported, and the interior patches are
   * drawn with the others, by the lit mesh and the material with the survey discard.
   */
  async #readyInterior(
    device: Device,
    kind: "lit-interior" | "lit-emissive-interior",
    path: TerrainLayout["vertexPath"],
  ): Promise<void> {
    try {
      const made = await this.#materialOf(kind, device, path);
      const held =
        kind === "lit-interior" ? device.interiorMaterial : device.emissiveInteriorMaterial;
      if (device !== this.#device || this.#disposed || held !== null) {
        this.#releaseOwn(made);
      } else if (kind === "lit-interior") {
        device.interiorMaterial = made;
      } else {
        device.emissiveInteriorMaterial = made;
      }
    } catch (error: unknown) {
      if (!this.#superseded(device) && !(error instanceof EngineUnavailable)) {
        console.error(
          "the terrain's interior form failed: its interior patches are drawn with the others",
          error,
        );
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s cluster path (plan R14, R14.T1.b and T1.e): the cull's kernel
   * and the interior patches' depth for the overlay; never rejects. Either that cannot be made is
   * reported, and the interior patches cast by the lit mesh and the overlay redraws its draws.
   */
  async #readyClusters(device: Device, path: TerrainLayout["vertexPath"]): Promise<void> {
    // A refusal thrown at the call, as a lost device's, settles as one rejected.
    const [kernel, depth] = await Promise.allSettled([
      (async () => this.#engine.createComputeAsync(this.#cullKernel))(),
      (async () => this.#materialOf("depth-cluster", device, path))(),
    ]);
    const taken =
      kernel.status === "fulfilled" &&
      depth.status === "fulfilled" &&
      device === this.#device &&
      !this.#disposed &&
      device.cullKernel === null &&
      device.clusterDepthMaterial === null;
    if (taken) {
      device.cullKernel = kernel.value;
      device.clusterDepthMaterial = depth.value;
      return;
    }
    // Neither is kept where either was refused: the one made is released.
    if (kernel.status === "fulfilled") {
      this.#engine.releaseCompute(kernel.value);
    }
    if (depth.status === "fulfilled") {
      this.#releaseOwn(depth.value);
    }
    for (const [outcome, name] of [
      [kernel, this.#cullKernel.name],
      [depth, "the cluster depth"],
    ] as const) {
      if (
        outcome.status === "rejected" &&
        !this.#superseded(device) &&
        !(outcome.reason instanceof EngineUnavailable)
      ) {
        this.#clusterRefusal ??= refusalOf(name, outcome.reason);
        console.error(
          "the terrain's cluster path failed: its interior patches cast by the lit mesh",
          outcome.reason,
        );
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s interior patches' lit form on the cluster path or its emissive
   * form (plan R14, R14.T2.a), and with the plain form the stable compaction's kernel; never
   * rejects. One that cannot be made is reported and its refusal stated
   * ({@link TerrainPass.clusterRefusal}), and the interior patches draw by the interior mesh.
   */
  async #readyLitClusters(
    device: Device,
    kind: "lit-cluster" | "lit-emissive-cluster",
    path: TerrainLayout["vertexPath"],
  ): Promise<void> {
    const plain = kind === "lit-cluster";
    // A refusal thrown at the call, as a lost device's, settles as one rejected.
    const [material, kernel] = await Promise.allSettled([
      (async () => this.#materialOf(kind, device, path))(),
      (async () => (plain ? this.#engine.createComputeAsync(CLUSTER_COMPACT) : null))(),
    ]);
    const current = device === this.#device && !this.#disposed;
    if (material.status === "fulfilled") {
      const held = plain ? device.litClusterMaterial : device.emissiveLitClusterMaterial;
      if (!current || held !== null) {
        this.#releaseOwn(material.value);
      } else if (plain) {
        device.litClusterMaterial = material.value;
      } else {
        device.emissiveLitClusterMaterial = material.value;
      }
    }
    if (kernel.status === "fulfilled" && kernel.value !== null) {
      if (!current || device.compactKernel !== null) {
        this.#engine.releaseCompute(kernel.value);
      } else {
        device.compactKernel = kernel.value;
      }
    }
    for (const [outcome, name] of [
      [material, `the ${kind} form`],
      [kernel, CLUSTER_COMPACT.name],
    ] as const) {
      if (
        outcome.status === "rejected" &&
        !this.#superseded(device) &&
        !(outcome.reason instanceof EngineUnavailable)
      ) {
        this.#clusterRefusal ??= refusalOf(name, outcome.reason);
        console.error(
          "the terrain's lit cluster path failed: its interior patches drawn by the interior mesh",
          outcome.reason,
        );
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s wireframe interior patches' form on the cluster path (plan R14,
   * R14.T4.c), with the cull's and the stable compaction's kernels; never rejects. Where any of
   * the three cannot be made, none is kept, the refusal is stated
   * ({@link TerrainPass.clusterRefusal}), and the wireframe draws every patch by the lit mesh.
   */
  async #readyWireClusters(device: Device, path: TerrainLayout["vertexPath"]): Promise<void> {
    // A refusal thrown at the call, as a lost device's, settles as one rejected.
    const [material, cull, compact] = await Promise.allSettled([
      (async () => this.#materialOf("wire-cluster", device, path))(),
      (async () => this.#engine.createComputeAsync(this.#cullKernel))(),
      (async () => this.#engine.createComputeAsync(CLUSTER_COMPACT))(),
    ]);
    const taken =
      material.status === "fulfilled" &&
      cull.status === "fulfilled" &&
      compact.status === "fulfilled" &&
      device === this.#device &&
      !this.#disposed &&
      device.wireClusterMaterial === null &&
      device.cullKernel === null &&
      device.compactKernel === null;
    if (taken) {
      device.wireClusterMaterial = material.value;
      device.cullKernel = cull.value;
      device.compactKernel = compact.value;
      return;
    }
    if (material.status === "fulfilled") {
      this.#releaseOwn(material.value);
    }
    for (const kernel of [cull, compact]) {
      if (kernel.status === "fulfilled") {
        this.#engine.releaseCompute(kernel.value);
      }
    }
    for (const [outcome, name] of [
      [material, "the wire-cluster form"],
      [cull, this.#cullKernel.name],
      [compact, CLUSTER_COMPACT.name],
    ] as const) {
      if (
        outcome.status === "rejected" &&
        !this.#superseded(device) &&
        !(outcome.reason instanceof EngineUnavailable)
      ) {
        this.#clusterRefusal ??= refusalOf(name, outcome.reason);
        console.error(
          "the wireframe's cluster path failed: its patches drawn by the lit mesh",
          outcome.reason,
        );
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s interior patches' caster on the cluster path (plan R14,
   * R14.T1.b); never rejects. One that cannot be made is reported, and they cast by the lit mesh.
   */
  async #readyClusterCasters(device: Device, path: TerrainLayout["vertexPath"]): Promise<void> {
    try {
      const caster = await this.#materialOf("shadow-cluster", device, path);
      if (device === this.#device && !this.#disposed && device.clusterShadowMaterial === null) {
        device.clusterShadowMaterial = caster;
      } else {
        this.#releaseOwn(caster);
      }
    } catch (error: unknown) {
      if (!this.#superseded(device) && !(error instanceof EngineUnavailable)) {
        this.#clusterRefusal ??= refusalOf("the cluster caster", error);
        console.error(
          "the terrain's cluster casters failed: its interior patches cast by the lit mesh",
          error,
        );
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s fin form of `kind` (plan R14, R14.T1.g): the edge patches'
   * may-discard clusters' lit form or its emissive form, their depth for the overlay or their
   * caster; never rejects. One that cannot be made is reported and its refusal stated
   * ({@link TerrainPass.clusterRefusal}), and the edge patches draw by the lit mesh.
   */
  async #readyFin(device: Device, kind: FinKind, path: TerrainLayout["vertexPath"]): Promise<void> {
    try {
      const made = await this.#materialOf(kind, device, path);
      if (device === this.#device && !this.#disposed && finMaterialOf(device, kind) === null) {
        setFinMaterial(device, kind, made);
      } else {
        this.#releaseOwn(made);
      }
    } catch (error: unknown) {
      if (!this.#superseded(device) && !(error instanceof EngineUnavailable)) {
        this.#clusterRefusal ??= refusalOf(`the ${kind} form`, error);
        console.error(
          "the terrain's fin clusters failed: its edge patches are drawn by the lit mesh",
          error,
        );
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s caster into the shadow cascades (R10.T9.b), for `depth-only`
   * with the lit mesh; never rejects. One that cannot be made is reported, and the pass draws on
   * without cascades.
   */
  async #readyShadow(device: Device, path: TerrainLayout["vertexPath"]): Promise<void> {
    try {
      const caster = await this.#materialOf("shadow", device, path);
      if (device === this.#device && !this.#disposed && device.shadowMaterial === null) {
        device.shadowMaterial = caster;
      } else {
        this.#releaseOwn(caster);
      }
    } catch (error: unknown) {
      if (!this.#superseded(device) && !(error instanceof EngineUnavailable)) {
        console.error("the terrain's shadow caster failed: the cascades are not drawn", error);
      }
    }
  }

  /**
   * Compiles, or takes, `device`'s lit material's emissive form (R08.T18.a), once a device; never
   * rejects. One that cannot be made is reported, and the plain form draws on, without the ground's
   * emission.
   */
  async #readyEmissive(device: Device, path: TerrainLayout["vertexPath"]): Promise<void> {
    try {
      const emissive = await this.#materialOf("lit-emissive", device, path);
      if (device === this.#device && !this.#disposed && device.emissiveMaterial === null) {
        device.emissiveMaterial = emissive;
      } else {
        this.#releaseOwn(emissive);
      }
    } catch (error: unknown) {
      if (!this.#superseded(device) && !(error instanceof EngineUnavailable)) {
        console.error(
          "the terrain's emissive form failed: its ground is drawn unlit by itself",
          error,
        );
      }
    }
  }

  /** Whether `device`'s materials are wanted no more: a restore replaced it, or the pass went. */
  #superseded(device: Device): boolean {
    return device !== this.#device || this.#disposed;
  }

  /**
   * The body's level table as its field's latest block gives it (`FieldChange.levels`, the pool's
   * `fieldLevels`; R10.T16.b), with its σ by level (`FieldChange.sigmaM`, the pool's
   * `fieldBounds`; R10.T4.b), which the selection bound reads under the statistical rule, and its
   * level charge (`FieldChange.chargeM`; R09.T8.d (b3)), which it reads under the hard rule:
   * selection reads its bounds from the next step on. A table, σ and charge equal to those held,
   * or no table, change nothing; no σ or charge is none.
   */
  setLevels(
    levels: Float64Array | null,
    sigmaM: Float64Array | null = null,
    chargeM: Float64Array | null = null,
  ): void {
    if (
      levels === null ||
      this.#disposed ||
      (sameLevels(this.#planet.levels, levels) &&
        sameLevels(this.#planet.sigmaM, sigmaM) &&
        sameLevels(this.#planet.chargeM, chargeM))
    ) {
      return;
    }
    this.#planet = planetGeometry(this.#planet.figure, levels, sigmaM, this.#planet.rule, chargeM);
    this.#rangesVersion += 1;
  }

  /**
   * Takes one block of the body's field (R10 Design note 16; decision D7): each resident patch
   * R10's rebake rule names (`needsRebake` of its bake's layers) is asked for again, at its last
   * request's priority, while its old bake stays drawn, and `settled` is called once every one of
   * them has landed in its slot, been evicted, or been given up; at once where the rule names none,
   * the pass draws none of the body's patches, or it is faulted or disposed.
   */
  fieldChanged(
    change: Pick<FieldBlockAnswer, "newlySurveyed" | "newlyHeld">,
    settled: () => void,
  ): void {
    const device = this.#device;
    if (this.#disposed || this.#faulted) {
      settled();
      return;
    }
    const keys = new Set<string>();
    for (const [keyString, resident] of device.residents) {
      // The lane asks no more for a key it gave up, so no gate may wait on one (S7).
      if (!device.givenUp.has(keyString) && needsRebake(resident.layers, change)) {
        keys.add(keyString);
        device.rebakes.set(keyString, resident.request);
      }
    }
    if (keys.size === 0) {
      settled();
      return;
    }
    device.gates.push({ keys, settled });
    this.#demandDirty = true;
  }

  /**
   * Halts the pass for good (plan R10, R10.T16.e; decision-r10-t16-leans §8): its session's terrain
   * computation kept failing, so, as when every height worker is lost, it asks for nothing more, its
   * demand withdrawn and every block's cells told, and draws nothing, each view's frame carrying
   * `cause`, its mask withheld ({@link TerrainPass.maskReadyFor}). Nothing for a pass already
   * faulted, whose first cause stands, or disposed.
   */
  halt(cause: TerrainFault): void {
    if (this.#disposed || this.#faulted) {
      return;
    }
    this.#fault(cause);
  }

  /**
   * One step, once a frame, over the views that hand the body over: selection when due (once for
   * them all), the demand and the rebakes to the pool, the draw set and the cache's pins. A view
   * that left since the last step releases its own draw resources, and the others keep theirs.
   *
   * @throws RangeError for more than {@link MAX_SELECTION_VIEWS} views.
   */
  step(input: TerrainStepInput): TerrainStep {
    return this.#step(input.views, input);
  }

  /** {@link TerrainPass.step} over `stepViews`, the spike's default view among them for `frame`. */
  #step(
    stepViews: ReadonlyArray<StepView>,
    input: Pick<TerrainStepInput, "grounded">,
  ): TerrainStep {
    if (stepViews.length > MAX_SELECTION_VIEWS) {
      throw new RangeError(
        `a terrain pass draws at most ${MAX_SELECTION_VIEWS} views, got ${stepViews.length}`,
      );
    }
    if (this.#disposed) {
      // Nothing is asked or drawn of a pass let go.
      return this.#faultedStep();
    }
    this.#leaveViews(stepViews);
    const device = this.#device;
    if (!this.#faulted && device.pool.unavailable === true) {
      this.#fault(TERRAIN_NOT_AVAILABLE);
    }
    if (this.#faulted) {
      return this.#faultedStep();
    }
    const views = stepViews.map((view) => this.#selectionView(view));
    const clock = this.#lookaheadClockMs;
    const nowMs = clock === null ? 0 : clock();
    this.#stepCostMs = 0;
    let reselected = false;
    if (this.#due(views, input)) {
      this.#select(this.#withMargins(views), input, nowMs);
      reselected = true;
    }
    for (const view of views) {
      this.#stepped.set(view.id, view.camera);
      const state = this.#views.get(view.id);
      if (state !== undefined && state.targetTauPx !== null) {
        const tauPx = slewedTauPx(state.bandTauPx, state.targetTauPx);
        if (tauPx !== state.bandTauPx || reselected) {
          state.bandTauPx = tauPx;
          this.#bandsOf(state, view);
        }
      }
    }
    if (clock !== null) {
      this.#steppedAtMs = nowMs;
      pushLatest(this.#stepCostsMs, this.#stepCostMs, LOOKAHEAD_COST_STEPS);
    }
    const selected = this.#selected;
    if (selected === null) {
      throw new Error("selection did not run");
    }
    if (!reselected && device.morph.swapDue) {
      // A coarsen's fade has reached 1: its patch takes its finer ones' place (R10.T16.i).
      const { selection, demand, resolved } = selected;
      this.#present(
        device,
        demand === selection.demand ? selection : { ...selection, demand },
        resolved,
      );
    }
    const { selection, drawSet } = selected;
    if (reselected || this.#demandDirty) {
      const demand = selected.demand.filter((r) => !device.cache.has(patchKeyString(r.key)));
      // The rebakes ask again for resident patches, which the selection's demand leaves out.
      const asked = [...demand, ...device.rebakes.values()];
      device.asked.clear();
      for (const request of asked) {
        device.asked.set(patchKeyString(request.key), request);
      }
      if (clock !== null) {
        this.#noteAsked(demand, device.asked, nowMs);
      }
      device.pool.reprioritise(asked);
      this.#demandDirty = false;
    }
    return { drawSet, selection, reselected };
  }

  /**
   * View `id`'s draw of the last step (R10.T16.b): its instance records, the drawn patches it sees
   * relative to its own camera at its own morph bands, its contact records and lights, its
   * conditions and debounced annunciation, and the fault where it stands. A view the last step did
   * not take draws nothing.
   *
   * @remarks
   * Where the view's body takes its shadow cascades on the high setting
   * ({@link TerrainFrameInput.takesCascades}; R10.T9.b), it fits them, and where their weight is
   * above 0 and its draw is made it renders their caster passes into the one set before it
   * returns, all four or, where it holds a far cascade from an earlier frame (plan R14, R14.T6.b),
   * those it does not hold: render this view's draw, in the bodies pass that follows, before
   * another view's `drawFor` draws its own cascades into the set.
   *
   * @throws RangeError if more contacts are given than the contact buffer holds.
   */
  drawFor(id: TerrainViewId, input: TerrainViewFrameInput): TerrainViewFrame {
    const selected = this.#selected;
    const state = this.#views.get(id);
    const bit = selected?.views.findIndex((view) => view.id === id) ?? -1;
    if (this.#faulted || this.#disposed || selected === null || state === undefined || bit < 0) {
      if (state !== undefined) {
        // This pass took no cascades of the view this frame (decision-r10-t9b-design D6(d)).
        state.centres = null;
        state.farHolds.cut();
        state.farPlan = null;
      }
      return {
        draws: NO_DRAWS,
        patches: NO_PATCHES,
        conditions: CLEAR,
        annunciation: null,
        fault: this.#faulted ? this.#cause : null,
        missing: false,
        standingIn: false,
        maskReady: false,
        cascades: null,
        overlayDepth: null,
        decorated: false,
      };
    }
    const { selection, drawSet } = selected;
    const view = selected.views[bit];
    const mask = 1 << bit;
    const missing = (drawSet.missingViews & mask) !== 0;
    const standingIn = (drawSet.standingInViews & mask) !== 0;
    const set = this.#cascadesFor(state, input);
    const { draws, patches, count, refused, cast, overlayDepth, overlayPass, lit } =
      this.#writeOrNone(state, mask, selected.nearContacts, input, set);
    const conditions: TerrainConditions = {
      streaming: missing || standingIn,
      // A view of weight w is limited where its τ′ is above τ (decision-r10-t16-contracts §3); the
      // low setting is coarser than the high one wherever terrain is drawn (Design note 26), but
      // for the wireframe's, whose τ and depth are the same on both settings (R10.T11).
      detailLimited:
        (selection.limited && selection.limitExcess > (view?.weight ?? 1)) ||
        (this.#setting === "low" &&
          TERRAIN_SHADING_TRAITS[this.#shading].coarserOnLow &&
          count > 0),
    };
    const annunciation = state.debounce.update(conditions, input.nowMs);
    if (cast.passes.length === 0 && !cast.reused.includes(true)) {
      // A frame that renders no cascades, and reuses none, is a cut of the far cascades' holds
      // (plan R14, R14.T6.b).
      state.farHolds.cut();
    }
    state.lastPatches = patches;
    state.lastCasters = set === null ? null : cast;
    state.lastOverlayPass = overlayPass;
    state.lastLit = lit;
    return {
      draws,
      patches,
      conditions,
      annunciation,
      fault: null,
      missing,
      standingIn,
      // A draw a loss refused at its making withholds the mask, whatever it would have drawn.
      maskReady:
        !missing && !refused && (draws.length > 0 || count === 0) && !this.#seesCodeZeroOpen(mask),
      cascades: set === null ? null : { set, received: state.farPlan?.received ?? set, ...cast },
      overlayDepth,
      decorated: false,
    };
  }

  /**
   * View `state`'s cascades this frame (plan R10, R10.T9.b; decision-r10-t9b-design D6), fitted
   * where its pass is lit, its setting draws cascades, its body takes them this frame and a star
   * lights it; `null` otherwise, and for a frame whose inputs cannot be fitted. The fit's snapped
   * centres are kept for the next frame's, and those of a frame with no fit are dropped.
   *
   * @remarks
   * The inputs, as the pass forms them: the camera's body-fixed place and orientation (as
   * selection takes them), the view's field and aspect; h, the camera's height above the datum
   * less the resident bakes' ground at its nadir (or less 0 where none is held, the reference
   * surface's datum: `heldGroundUnder`), and ν, the datum's normal there; the top edge of star 0's
   * disc, the lit rows' brightest, its body-fixed direction raised by its angular radius ρ towards
   * ν ({@link discTopDirection}; decision-r10-t9b-penumbra), so that the cascades' shadow begins
   * where the horizon map's share m reaches 0 and min(m, c′) is m across the penumbra; R, the
   * figure's mean radius; Δh, the body's realised relief, 0 where it is not finite; the horizon
   * texel from the view's morph bands ({@link cascadeHorizonTexelM}); and the device's
   * `depth-clip-control`. A frame at w = 0 still fits, so that its centres carry. The receiver's
   * N·L, its normal offset and the shading take star 0's own direction from the lights' rows, not
   * the fit's.
   *
   * A frame whose inputs cannot be fitted (a viewport of no area or not finite, a star of zero or
   * non-finite direction or angular radius, or any input `fitCascades` refuses) takes no cascades,
   * as a frame of no star does, rather than throwing out of {@link TerrainPass.drawFor}.
   */
  #cascadesFor(state: ViewState, input: TerrainViewFrameInput): CascadeSet | null {
    const previous = state.centres;
    state.centres = null;
    state.farPlan = null;
    const star = input.lighting?.lights[0];
    if (
      this.#shading !== "lit" ||
      this.#shadows?.cascades !== true ||
      input.takesCascades !== true ||
      star === undefined
    ) {
      return null;
    }
    const { rotation, cameraM, orientation, fovXRad, viewport } = input.view;
    const aspect = viewport.widthPx / viewport.heightPx;
    const sunLength = norm(star.direction);
    const rhoRad = Math.asin(Math.min(1, star.radius / star.distance));
    if (
      !(viewport.widthPx > 0 && viewport.heightPx > 0 && aspect > 0 && Number.isFinite(aspect)) ||
      !(sunLength > 0 && Number.isFinite(sunLength)) ||
      !(rhoRad >= 0)
    ) {
      return null;
    }
    const relief = this.#reliefM();
    const camera = rotateToBodyFixed(rotation, cameraM);
    const planet = this.#planet;
    const geometry = this.#geometry;
    let fit: CascadeInput;
    let set: CascadeSet;
    try {
      const under = heldGroundUnder(planet.figure, this.#heights)(camera);
      fit = {
        cameraBodyFixedM: camera,
        orientationBodyFixed: multiply(conjugate(quaternionFromRows(rotation.rows)), orientation),
        rotation,
        fovXRad,
        aspect,
        heightAboveGroundM: under.heightM,
        sunDirectionBodyFixed: discTopDirection(star.direction, under.normal, rhoRad),
        datumNormalBodyFixed: under.normal,
        bodyRadiusM: meanRadiusM(planet.figure),
        reliefRangeM: relief !== null && Number.isFinite(relief) ? Math.max(0, relief) : 0,
        horizonTexelM: cascadeHorizonTexelM(planet, state.morph, under.heightM),
        depthClipControl: this.#engine.capabilities.depthClipControl,
        ...(previous === null ? {} : { previousCentresBodyFixedM: previous }),
        // Unread by the way back, as every lever (plan R14, R14.T6.b).
        farCascadeRate: geometry.clusterPath ? geometry.farCascadeRate : 1,
      };
      set = fitCascades(fit);
    } catch (error: unknown) {
      // The fit refuses what it cannot fit, a camera not finite, say: no cascades this frame.
      if (error instanceof RangeError) {
        return null;
      }
      throw error;
    }
    const [c0, c1, c2, c3] = set.cascades;
    state.centres = [
      c0.centreBodyFixedM,
      c1.centreBodyFixedM,
      c2.centreBodyFixedM,
      c3.centreBodyFixedM,
    ];
    const shadows = this.#shadows;
    const owner = state.farHolds;
    state.farPlan = owner.plan({
      input: fit,
      set,
      sunAngularRadiusRad: rhoRad,
      depthCull: geometry.casterDepthCull,
      holdable: shadows.casters.size === 0,
      kept: (index) => shadows.holds(owner, index),
    });
    return set;
  }

  /**
   * Whether view `id` sees a drawn patch at the last step: no fault, the view taken by that step,
   * and some patch of the draw set in its view. Where it holds, the view's draw draws the terrain
   * once its material is made; where it does not, the view sees no surveyed ground the pass draws
   * (past the survey, or every patch it sees still missing). The views check's terrain run reads it
   * (R10.T16.d), so that a view past the survey's edge is not taken for one whose terrain failed.
   */
  seesPatch(id: TerrainViewId): boolean {
    const selected = this.#selected;
    if (this.#faulted || this.#disposed || selected === null || !this.#views.has(id)) {
      return false;
    }
    const bit = selected.views.findIndex((view) => view.id === id);
    if (bit < 0) {
      return false;
    }
    const mask = 1 << bit;
    const { drawSet } = selected;
    for (let n = 0; n < drawSet.count; n += 1) {
      if (((drawSet.views[n] ?? 0) & mask) !== 0) {
        return true;
      }
    }
    return false;
  }

  /**
   * Whether view `id`'s frame may give the body's coverage mask (decision D7), as the last step
   * leaves it, before the view's draw: no fault, the view taken by that step, nothing it sees
   * missing, no drawn patch it sees of survey cell code 0 whose bake holds a vertex outside the
   * survey (decision-r10-t16-review §2: its vertices cannot hold its cells), and its draw ready to
   * be made wherever it sees a patch (the material compiled, and a lit pass's phase table and
   * tables of no air made). {@link TerrainPass.drawFor}'s `maskReady` is this, but for a draw
   * refused at its making (a device lost meanwhile); a view's frame reads this one before its plan,
   * which takes the mask (R10.T16.c).
   */
  maskReadyFor(id: TerrainViewId): boolean {
    const selected = this.#selected;
    if (this.#faulted || this.#disposed || selected === null || !this.#views.has(id)) {
      return false;
    }
    const bit = selected.views.findIndex((view) => view.id === id);
    if (bit < 0) {
      return false;
    }
    const mask = 1 << bit;
    const { drawSet } = selected;
    if ((drawSet.missingViews & mask) !== 0 || this.#seesCodeZeroOpen(mask)) {
      return false;
    }
    let sees = false;
    for (let n = 0; n < drawSet.count && !sees; n += 1) {
      sees = ((drawSet.views[n] ?? 0) & mask) !== 0;
    }
    const device = this.#device;
    return (
      !sees ||
      (device.material !== null &&
        (this.#shading !== "lit" ||
          (device.resources.phaseTable !== null && device.airless !== null)))
    );
  }

  /**
   * Whether the views of `mask` see a drawn patch, selected or standing in, of survey cell code 0
   * whose last bake holds a vertex outside the survey (decision-r10-t16-review §2): one is drawn as
   * the stream's first bake and first stand-in at whatever range the camera holds, and where its
   * cells lie between its vertices neither surface would draw them, so the mask is withheld while
   * it is drawn; it raises no `TERRAIN: STREAMING` of its own.
   */
  #seesCodeZeroOpen(mask: number): boolean {
    const drawSet = this.#selected?.drawSet;
    if (drawSet === undefined) {
      return false;
    }
    const { residents } = this.#device;
    for (let n = 0; n < drawSet.count; n += 1) {
      const drawn = drawSet.patches[n];
      if (
        drawn !== undefined &&
        ((drawSet.views[n] ?? 0) & mask) !== 0 &&
        residents.get(drawn.keyString)?.codeZeroOpen === true
      ) {
        return true;
      }
    }
    return false;
  }

  /**
   * Runs one frame of the descent spike's single view, {@link DEFAULT_TERRAIN_VIEW}: a step at the
   * terrain's τ and weight 1, then its draw, as R05 built it.
   *
   * @throws RangeError if more contacts are given than the contact buffer holds.
   */
  frame(input: TerrainFrameInput): TerrainFrame {
    const { drawSet, selection, reselected } = this.#step(
      [{ id: DEFAULT_TERRAIN_VIEW, view: input.view, tauPx: this.#terrain.tauPx, weight: 1 }],
      input,
    );
    const { draws, conditions, annunciation } = this.drawFor(DEFAULT_TERRAIN_VIEW, input);
    return { draws, drawSet, selection, reselected, conditions, annunciation };
  }

  /**
   * Releases view `id`'s own draw resources, as a view that no longer hands the body over; only a
   * step that leaves the view out does it, so that a view taken again always takes a selection,
   * and with it its morph bands (the batch review's C9).
   */
  #forgetView(id: TerrainViewId): void {
    this.#stepped.delete(id);
    this.#nearestGround.delete(id);
    this.#views.get(id)?.clusters?.stats?.dispose();
    if (this.#views.delete(id)) {
      this.#device.resources.releaseView(id);
    }
  }

  /**
   * Stops the pool and following restores, tells every block still waiting for its rebakes, and
   * releases every handle the pass made on the current device: its resources and every view's, its
   * mesh, the materials it compiled itself (not the session's, R10.T14.d), its tables of no air
   * (decision-r10-t16-contracts §1) and its own shadow cascades (not the session's, R10.T9.b). A
   * caller never disposes of a pass a frame not yet submitted draws.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    const device = this.#device;
    try {
      this.#settleAll(device);
    } finally {
      // Whatever a block's listener threw, the lane ends and every handle goes (S3, S4).
      this.#offRestored();
      device.offBaked();
      device.offFailed();
      device.pool.terminate();
      for (const state of this.#views.values()) {
        state.clusters?.stats?.dispose();
      }
      this.#views.clear();
      this.#stepped.clear();
      this.#nearestGround.clear();
      this.#heights.clear();
      try {
        device.resources.dispose();
      } finally {
        try {
          this.#releaseDevice(device);
        } finally {
          if (this.#ownShadows) {
            this.#shadows?.dispose();
          }
        }
      }
    }
  }

  /** The materials the pass compiled itself and the tables of no air of `device`. */
  #releaseDevice(device: Device): void {
    if (device.material !== null) {
      this.#releaseOwn(device.material);
      device.material = null;
    }
    if (device.depthMaterial !== null) {
      this.#releaseOwn(device.depthMaterial);
      device.depthMaterial = null;
    }
    if (device.shadowMaterial !== null) {
      this.#releaseOwn(device.shadowMaterial);
      device.shadowMaterial = null;
    }
    if (device.emissiveMaterial !== null) {
      this.#releaseOwn(device.emissiveMaterial);
      device.emissiveMaterial = null;
    }
    if (device.interiorMaterial !== null) {
      this.#releaseOwn(device.interiorMaterial);
      device.interiorMaterial = null;
    }
    if (device.emissiveInteriorMaterial !== null) {
      this.#releaseOwn(device.emissiveInteriorMaterial);
      device.emissiveInteriorMaterial = null;
    }
    if (device.clusterShadowMaterial !== null) {
      this.#releaseOwn(device.clusterShadowMaterial);
      device.clusterShadowMaterial = null;
    }
    if (device.clusterDepthMaterial !== null) {
      this.#releaseOwn(device.clusterDepthMaterial);
      device.clusterDepthMaterial = null;
    }
    if (device.cullKernel !== null) {
      this.#engine.releaseCompute(device.cullKernel);
      device.cullKernel = null;
    }
    if (device.litClusterMaterial !== null) {
      this.#releaseOwn(device.litClusterMaterial);
      device.litClusterMaterial = null;
    }
    if (device.emissiveLitClusterMaterial !== null) {
      this.#releaseOwn(device.emissiveLitClusterMaterial);
      device.emissiveLitClusterMaterial = null;
    }
    if (device.compactKernel !== null) {
      this.#engine.releaseCompute(device.compactKernel);
      device.compactKernel = null;
    }
    if (device.wireClusterMaterial !== null) {
      this.#releaseOwn(device.wireClusterMaterial);
      device.wireClusterMaterial = null;
    }
    for (const kind of FIN_KINDS) {
      const held = finMaterialOf(device, kind);
      if (held !== null) {
        this.#releaseOwn(held);
        setFinMaterial(device, kind, null);
      }
    }
    if (device.airless !== null) {
      this.#engine.releaseTexture(device.airless.body);
    }
  }

  #makeDevice(resources?: TerrainResources): Device {
    const made =
      resources ??
      new TerrainResources(
        this.#engine,
        this.#terrain,
        this.#planet.figure,
        this.#shading,
        // A lit pass's interior patches cast and give the overlay its depth on the cluster path, and
        // the wireframe's draw on it (plan R14, R14.T4.c).
        this.#shading === "lit" || this.#shading === "wire",
      );
    const { layout } = made;
    const cache = new PatchCache(layout.slots);
    const pool = this.#createPool({
      vertexPath: layout.vertexPath,
      normals: this.#terrain.normals,
      source: this.#source,
    });
    const device: Device = {
      resources: made,
      cache,
      resolver: new DrawSetResolver(cache),
      holds: new CoarseFineHolds(layout.slots.slotCount),
      morph: new TimeMorph(
        layout.slots.slotCount,
        this.#morphInMs,
        this.#morphSpans,
        this.#shading !== "wire",
      ),
      fades: new TransitionFades(layout.slots.slotCount),
      slotVersions: new Uint32Array(layout.slots.slotCount),
      fadesAtMs: null,
      firstDrawn: false,
      drawnKeys: [],
      drawnLevels: [],
      pool,
      offBaked: pool.onBaked((bake) => {
        this.#store(device, bake);
      }),
      offFailed:
        pool.onFailed?.((failure) => {
          this.#givenUp(device, failure.keyString);
        }) ?? ((): void => undefined),
      offRefined:
        pool.onRefined?.((refined) => {
          this.#refined(device, refined);
        }) ?? ((): void => undefined),
      drafts: new Map(),
      airless:
        this.#shading === "lit" ? createAirlessTables(this.#engine, "terrain: no air") : null,
      material: null,
      depthMaterial: null,
      shadowMaterial: null,
      emissiveMaterial: null,
      emissiveAsked: false,
      interiorMaterial: null,
      emissiveInteriorMaterial: null,
      litClusterMaterial: null,
      emissiveLitClusterMaterial: null,
      compactKernel: null,
      litFinMaterial: null,
      emissiveLitFinMaterial: null,
      finDepthMaterial: null,
      finShadowMaterial: null,
      wireClusterMaterial: null,
      clusterShadowMaterial: null,
      clusterDepthMaterial: null,
      cullKernel: null,
      residents: new Map(),
      rebakes: new Map(),
      gates: [],
      asked: new Map(),
      givenUp: new Set(),
      graticules: new Map(),
    };
    return device;
  }

  /** After a device loss: the same resources object, re-made; a new cache, pool and material. */
  #rebuild(): void {
    const old = this.#device;
    old.offBaked();
    old.offFailed();
    old.offRefined();
    old.pool.terminate();
    // Nothing is resident now: every view's draw set misses its patches, so no mask is given.
    this.#settleAll(old);
    // The lost device's handles went with it: `ResilientEngine` drops their release.
    this.#releaseDevice(old);
    this.#heights.clear();
    this.#device = this.#makeDevice(old.resources);
    // The new device's cluster path is made again, and refused again only if its compiler says so.
    this.#clusterRefusal = null;
    // Each view's records sized to the new layout's slots, its debounce kept; its cascades'
    // centres dropped, since the pass is remade (decision-r10-t9-split, ruling 7 amended).
    for (const [id, state] of this.#views) {
      // The lost device's counters went with it; the view's next measured frame makes its own.
      state.clusters?.stats?.dispose();
      this.#views.set(id, { ...this.#viewState(id), debounce: state.debounce });
    }
    this.#rangesVersion += 1;
    this.#selected = null;
    this.#demandDirty = true;
    // The new device's cache holds nothing yet: its demand is timed afresh.
    this.#askedAtMs.clear();
    this.#aheadDemand = null;
    void this.ready().catch((error: unknown) => {
      console.error("the terrain material failed after a restore", error);
    });
  }

  /**
   * Every height worker is lost, or the session halted the pass: no demand, no draw, and every
   * block's cells told; its frames carry `cause`.
   */
  #fault(cause: TerrainFault): void {
    this.#faulted = true;
    this.#cause = cause;
    const device = this.#device;
    device.pool.reprioritise([]);
    device.rebakes.clear();
    this.#settleAll(device);
  }

  /** A faulted or disposed pass's step: the draw set empty, the last selection kept. */
  #faultedStep(): TerrainStep {
    const selection = this.#selected?.selection ?? EMPTY_SELECTION;
    return {
      drawSet: this.#device.resolver.resolve(EMPTY_SELECTION),
      selection,
      reselected: false,
    };
  }

  #store(device: Device, bake: BakedPatch): void {
    if (device !== this.#device || this.#disposed) {
      return;
    }
    const keyString = patchKeyString(bake.key);
    // A rebake of a drawn patch takes a spare slot, where one is, so that the older bake fades out
    // from its own (R10.T16.i); the wireframe's replaces it in place, its contours stating the
    // elevations of one bake or the other, never a mix.
    const rebake = device.residents.has(keyString) && this.#shading !== "wire";
    const sizes = bakeSizesOf(bake);
    let retired: number | null = null;
    let stored: CacheInsert;
    if (rebake) {
      const replaced = device.cache.replace(bake);
      retired = replaced.kind === "stored" ? replaced.retired : null;
      stored = replaced;
    } else {
      stored = device.cache.insert(bake);
    }
    if (stored.kind !== "stored") {
      const observers = this.#observers;
      for (let n = 0; n < observers.length; n += 1) {
        observers[n]?.onRefused?.(bake.key);
      }
      return;
    }
    if (stored.evicted !== null) {
      this.#forget(device, stored.evicted);
    }
    if (rebake) {
      const older = device.residents.get(keyString)?.sizes;
      const changeM = older === undefined ? sizes.reliefM : coarseChangeM(older, sizes);
      device.morph.rebake(keyString, retired, changeM);
      this.#releaseRetired(device);
    }
    let upload: ReturnType<TerrainResources["upload"]>;
    try {
      upload = device.resources.upload({ slot: stored.slot, ...bake });
    } catch (error: unknown) {
      if (!(error instanceof EngineUnavailable)) {
        throw error;
      }
      // A bake landing during a loss, whose phase table could not grow: the restore's rebuild
      // asks for it again.
      upload = { kind: "refused", reason: "slot-outside-layout" };
    }
    if (upload.kind !== "uploaded") {
      device.cache.remove(keyString);
      this.#forget(device, keyString);
      return;
    }
    device.slotVersions[stored.slot] = ((device.slotVersions[stored.slot] ?? 0) + 1) >>> 0;
    // A rebake replaces its old bake in its slot (`PatchCache.insert` of a resident key).
    this.#heights.take(bake);
    const layers = bake.layers;
    device.givenUp.delete(keyString);
    device.residents.set(keyString, {
      layers:
        layers === null
          ? null
          : {
              complete: layers.complete,
              unsurveyedCells: layers.unsurveyedCells,
              horizonRefusedCells: layers.horizonRefusedCells,
            },
      // The request it was last asked with: the last demand's, its rebake's, or, for a bake that
      // lands outside any, the one it was held with before.
      request: device.asked.get(keyString) ??
        device.rebakes.get(keyString) ??
        device.residents.get(keyString)?.request ?? { key: bake.key, priority: 0, forced: false },
      codeZeroOpen:
        layers !== null &&
        layers.unsurveyedCells.length > 0 &&
        surveyCellCode(bake.key.level, layers.fieldLevel) === 0,
      originHeightM: Math.fround(bake.originHeightM),
      interior: isInteriorBake(layers),
      sizes,
    });
    device.graticules.delete(keyString);
    device.rebakes.delete(keyString);
    this.#draft(device, bake, keyString);
    this.#landed(device, keyString);
    const askedAtMs = this.#askedAtMs.get(keyString);
    if (askedAtMs !== undefined && this.#lookaheadClockMs !== null) {
      this.#askedAtMs.delete(keyString);
      pushLatest(
        this.#roundTripsS,
        (this.#lookaheadClockMs() - askedAtMs) / 1000,
        LOOKAHEAD_SAMPLES,
      );
    }
    this.#rangesVersion += 1;
    this.#demandDirty = true;
    this.#onResident?.(bake.key);
    const observers = this.#observers;
    for (let n = 0; n < observers.length; n += 1) {
      observers[n]?.onResident?.(bake.key);
    }
  }

  /**
   * Keeps a drafted bake's vertices' normals and asks for its refinement, at its request's priority;
   * a whole bake drops any refinement of the draft it replaced.
   */
  #draft(device: Device, bake: BakedPatch, keyString: string): void {
    const { pool } = device;
    if (bake.drafted === true && pool.refine !== undefined) {
      this.#tickets += 1;
      const ticket = this.#tickets;
      device.drafts.set(keyString, { vertexBits: draftVertexBits(bake.normals), ticket });
      const priority = device.residents.get(keyString)?.request.priority ?? 0;
      pool.refine({ key: bake.key, priority, ticket });
    } else if (device.drafts.delete(keyString)) {
      pool.dropRefinement?.(keyString);
    }
  }

  /**
   * A draft's refined normals: its slot's normals made the whole bake's and written alone, and its
   * completeness the draft's and theirs, which R10's rebake rule reads; dropped unless the draft
   * they refine is still the one held.
   */
  #refined(device: Device, refined: RefinedDraft): void {
    if (device !== this.#device || this.#disposed) {
      return;
    }
    const keyString = patchKeyString(refined.key);
    const draft = device.drafts.get(keyString);
    const held = device.cache.get(keyString);
    if (draft === undefined || draft.ticket !== refined.ticket || held === undefined) {
      return;
    }
    device.drafts.delete(keyString);
    const normals = mergeRefinedNormals(draft.vertexBits, refined.normals);
    let written: ReturnType<TerrainResources["uploadNormals"]>;
    try {
      written = device.resources.uploadNormals(held.slot, normals);
    } catch (error: unknown) {
      if (!(error instanceof EngineUnavailable)) {
        throw error;
      }
      // Refined during a loss: the restore's rebuild bakes the patch again.
      return;
    }
    const resident = device.residents.get(keyString);
    const layers = resident?.layers ?? null;
    if (
      written.kind === "uploaded" &&
      !refined.complete &&
      resident !== undefined &&
      layers !== null
    ) {
      device.residents.set(keyString, { ...resident, layers: { ...layers, complete: false } });
    }
  }

  /** A patch no longer resident: evicted, or refused by its slot. */
  #forget(device: Device, keyString: string): void {
    if (device.drafts.delete(keyString)) {
      device.pool.dropRefinement?.(keyString);
    }
    device.residents.delete(keyString);
    device.givenUp.delete(keyString);
    device.graticules.delete(keyString);
    this.#heights.forget(keyString);
    // A rebake fading from the older bake ends with its patch: a bake of the key stored again takes
    // no stale older slot.
    device.morph.forget(keyString);
    this.#releaseRetired(device);
    if (device.rebakes.delete(keyString)) {
      this.#demandDirty = true;
    }
    this.#landed(device, keyString);
  }

  /** A rebake the pool gave up: the old bake stays drawn, and its block is told. */
  #givenUp(device: Device, keyString: string): void {
    if (device !== this.#device) {
      return;
    }
    if (device.residents.has(keyString)) {
      device.givenUp.add(keyString);
    }
    if (device.rebakes.delete(keyString)) {
      this.#demandDirty = true;
    }
    this.#landed(device, keyString);
  }

  /** `keyString`'s rebake is settled: each block waiting on it alone is told, oldest first. */
  #landed(device: Device, keyString: string): void {
    const gates = device.gates;
    for (let n = 0; n < gates.length;) {
      const gate = gates[n];
      if (gate === undefined) {
        break;
      }
      gate.keys.delete(keyString);
      if (gate.keys.size === 0) {
        gates.splice(n, 1);
        gate.settled();
      } else {
        n += 1;
      }
    }
  }

  /** Tells every block still waiting on `device`'s rebakes, oldest first. */
  #settleAll(device: Device): void {
    const gates = device.gates.splice(0);
    for (const gate of gates) {
      gate.settled();
    }
  }

  /** Forgets the views that no longer hand the body over, and keeps a state for each that does. */
  #leaveViews(views: ReadonlyArray<StepView>): void {
    for (const id of this.#views.keys()) {
      if (!views.some((view) => view.id === id)) {
        this.#forgetView(id);
      }
    }
    for (const { id } of views) {
      if (!this.#views.has(id)) {
        this.#views.set(id, this.#viewState(id));
      }
    }
  }

  #viewState(id: TerrainViewId): ViewState {
    const bodyRotation = new Float32Array(16);
    const atlas = new Float32Array(4);
    const cameraBodyFixedM = new Float32Array(4);
    const { slotCount } = this.#device.resources.layout.slots;
    return {
      id,
      debounce: new TerrainAnnunciationDebounce(),
      instances: new InstanceRecords(slotCount),
      contacts: new ContactRecords(),
      morph: new Float64Array(2 * (MAX_LEVEL + 1)),
      bandTauPx: null,
      targetTauPx: null,
      uniforms: {
        bodyRotation,
        sunDirection: new Float32Array(4),
        sunRadiance: new Float32Array(4),
        atlas,
      },
      litUniforms: { bodyRotation, cameraBodyFixedM, atlas, ...cascadeUniforms() },
      casterUniforms: { bodyRotation, cameraBodyFixedM, atlas },
      centres: null,
      farHolds: new FarCascadeHolds(),
      farPlan: null,
      casterInputs: new CascadeReuse(),
      drawn: [],
      lastPatches: NO_PATCHES,
      lastCasters: null,
      lastOverlayPass: null,
      lastLit: false,
      clusters: null,
      interior: new InstanceRecords(slotCount),
      interiorDrawn: [],
      interiorFins: [],
      interiorFading: [],
      interiorDraw: null,
      interiorBeside: null,
      casters: null,
      drawnShadow: null,
      wireUniforms: {
        bodyRotation,
        atlas: new Float32Array(4),
        contour: new Float32Array(4),
        contourWidthsPx: new Float32Array(4),
        graticule: new Float32Array(4),
        muted: new Float32Array(4),
        text: new Float32Array(4),
        casing: new Float32Array(4),
        occluderSlopePx: new Float32Array(1),
      },
      phases: TERRAIN_SHADING_TRAITS[this.#shading].phases ? new WirePhaseRecords(slotCount) : null,
      interiorPhases: TERRAIN_SHADING_TRAITS[this.#shading].phases
        ? new WirePhaseRecords(slotCount)
        : null,
      draw: null,
      drawDevice: null,
      drawnTable: null,
      drawnAtmosphere: null,
    };
  }

  /** Rewrites `state`'s morph bands for `view` at its slewed tolerance. */
  #bandsOf(state: ViewState, view: PassSelectionView): void {
    const morphView = { ...view, tauPx: state.bandTauPx ?? view.bandTauPx };
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      const [start, end] = morphRangeM(this.#planet, level, morphView);
      state.morph[2 * level] = start;
      state.morph[2 * level + 1] = end;
    }
  }

  /** A view as selection takes it: body-fixed, at τ_sel ({@link selectionTolerancePx}). */
  #selectionView(view: StepView): PassSelectionView {
    const { rotation, cameraM, orientation } = view.view;
    const qBody = quaternionFromRows(rotation.rows);
    return {
      id: view.id,
      bandTauPx: view.tauPx,
      camera: {
        positionM: rotateToBodyFixed(rotation, cameraM),
        orientation: multiply(conjugate(qBody), orientation),
      },
      fovXRad: view.view.fovXRad,
      viewport: view.view.viewport,
      weight: view.weight,
      tauPx: selectionTolerancePx(view.tauPx),
    };
  }

  #due(
    views: ReadonlyArray<PassSelectionView>,
    input: Pick<TerrainStepInput, "grounded">,
  ): boolean {
    const last = this.#selected;
    if (last === null) {
      return true;
    }
    if (
      last.views.length !== views.length ||
      last.rangesVersion !== this.#rangesVersion ||
      last.surveyVersion !== (this.#survey?.version ?? 0) ||
      !sameContacts(last.grounded, input.grounded)
    ) {
      return true;
    }
    return views.some((view, n) => {
      const was = last.views[n];
      if (
        was === undefined ||
        was.id !== view.id ||
        was.fovXRad !== view.fovXRad ||
        was.viewport.widthPx !== view.viewport.widthPx ||
        was.viewport.heightPx !== view.viewport.heightPx ||
        was.tauPx !== view.tauPx ||
        was.weight !== view.weight
      ) {
        return true;
      }
      // The whole rotation, roll included: 2 acos |q · q′| is the angle between two orientations.
      const q = view.camera.orientation;
      const p = was.orientationBodyFixed;
      const dot = Math.abs(q.w * p.w + q.x * p.x + q.y * p.y + q.z * p.z);
      const pixelRad = view.fovXRad / view.viewport.widthPx;
      if (2 * Math.acos(Math.min(dot, 1)) > pixelRad) {
        return true;
      }
      // With no coarse patch in view, its own nearest surveyed ground (+∞ with no survey, as
      // R05's), not another view's (S9).
      const nearestM = Number.isFinite(was.nearestM)
        ? was.nearestM
        : (last.selection.nearestSurveyByViewM[n] ?? last.selection.nearestSurveyM);
      // m ÷ (1 + m) of d_min, not m, so that the drawn error stays within τ, not τ ÷ (1 − m²).
      return (
        distance(view.camera.positionM, was.cameraBodyFixedM) > RESELECT_MOVE_FRACTION * nearestM
      );
    });
  }

  /**
   * `views` with each one's frustum margin under `motion` ({@link frustumMarginRad}), from its
   * camera at the last step and its nearest ground: the nearest a selection last gave it, and no
   * nearer than the camera's height above the drawn ground beneath it; unchanged under `none`.
   *
   * @remarks
   * The margin's ground was the last selection's, and so fed back into the next one's, which
   * cycled at TERRAIN TEST's survey edge (the seamless lane, the owner's priority, 2026-11-01).
   * After a selection that gave a view no ground, its survey's nearest stood in, a finest patch
   * with the camera over the survey and the survey out of view: that opened the margin to its most,
   * and the ground it then selected, far, closed it again, so that the selection gave nothing and
   * some 300 patches by turns, frame after frame (high, 253.7 to 254.5 s). A coarse patch whose box
   * holds the camera, its range spanning the camera's height, gave the same finest patch, and the
   * finer patches that replaced it under the opened margin a far ground (low, 252.55 to 252.8 s: 76
   * and 187 patches by turns). The last ground selected is carried past a selection that gives
   * none, and the drawn ground beneath the camera, from the resident bakes, floors it at a
   * distance no box makes, so that neither feeds back; the survey's nearest stands in only for a
   * view never given ground.
   */
  #withMargins(views: ReadonlyArray<PassSelectionView>): ReadonlyArray<PassSelectionView> {
    if (this.#frustumMargin === "none") {
      return views;
    }
    const last = this.#selected;
    return views.map((view) => {
      const n = last?.views.findIndex((was) => was.id === view.id) ?? -1;
      const nearestM = Math.max(
        this.#nearestGround.get(view.id) ??
          last?.selection.nearestSurveyByViewM[n] ??
          Number.POSITIVE_INFINITY,
        this.#heightAboveGroundM(view.camera.positionM),
      );
      return {
        ...view,
        frustumMarginRad: frustumMarginRad(view, this.#stepped.get(view.id) ?? null, nearestM),
      };
    });
  }

  /**
   * The camera's height above the drawn ground beneath it, metres, at `cameraM` from the body's
   * centre, body-fixed: along its direction to the finest resident bake's height there; 0 where no
   * bake is resident beneath it.
   */
  #heightAboveGroundM(cameraM: Vec3): number {
    const r = norm(cameraM);
    if (!(r > 0)) {
      return 0;
    }
    const direction = { x: cameraM.x / r, y: cameraM.y / r, z: cameraM.z / r };
    const groundHeightM = this.#heights.groundHeightAtM(direction);
    if (groundHeightM === null) {
      return 0;
    }
    const [x, y, z] = surfacePoint(
      this.#planet.figure,
      [direction.x, direction.y, direction.z],
      groundHeightM,
    );
    return r - Math.hypot(x, y, z);
  }

  #select(
    views: ReadonlyArray<PassSelectionView>,
    input: Pick<TerrainStepInput, "grounded">,
    nowMs: number,
  ): void {
    const planet = this.#planet;
    const last = this.#selected?.selection;
    const holdMargin = this.#budgetHoldMargin;
    const selectionInput: SelectionInput = {
      planet,
      // Selection's own view inputs, without the pass's names for them.
      views: views.map(
        ({
          camera,
          fovXRad,
          viewport,
          weight,
          tauPx,
          frustumMarginRad: margin,
        }): ViewSelectionInput => ({
          camera,
          fovXRad,
          viewport,
          weight,
          tauPx,
          ...(margin === undefined ? {} : { frustumMarginRad: margin }),
        }),
      ),
      setting: this.#setting,
      grounded: input.grounded,
      maxPatches: this.maxPatches,
      heightRanges: this.#device.cache,
      skirtMarginM: WORKER_SKIRT_MARGIN_M,
      ...(this.#survey === null ? {} : { survey: this.#survey }),
      ...(holdMargin === null || last === undefined
        ? {}
        : { budgetHold: { split: splitKeysOf(last.patches.values()), margin: holdMargin } }),
      ...(this.#surveyedFloorLevel > 0 ? { surveyedFloorLevel: this.#surveyedFloorLevel } : {}),
    };
    // The lookahead's cadence reads the selection's cost too.
    const timed = this.#measureSelection || this.#lookaheadClockMs !== null;
    const startMs = timed ? performance.now() : 0;
    const selection = selectPatches(selectionInput);
    if (timed) {
      const endMs = performance.now();
      if (this.#measureSelection) {
        performance.measure(SELECT_MEASURE, { start: startMs, end: endMs });
      }
      this.#stepCostMs += endMs - startMs;
    }
    this.#onSelect?.(selectionInput, selection);
    const observers = this.#observers;
    for (let n = 0; n < observers.length; n += 1) {
      observers[n]?.onSelect?.(selectionInput, selection);
    }
    const patchM = finestPatchSizeM(planet);
    const selectedViews = views.map((view, n): SelectedView => {
      const mask = 1 << n;
      let nearestM = Number.POSITIVE_INFINITY;
      for (const patch of selection.patches.values()) {
        if (patch.key.level < planet.finestLevel && (patch.views & mask) !== 0) {
          // The box, as selection measures a patch's distance: a coarse patch's bounding sphere
          // often holds the camera, which would clamp the rule to one finest patch (lane B).
          nearestM = Math.min(
            nearestM,
            distanceToBoxM(relativeBounds(patch.bounds, view.camera.positionM)),
          );
        }
      }
      if (Number.isFinite(nearestM)) {
        this.#nearestGround.set(view.id, Math.max(nearestM, patchM));
      }
      // At the view's τ, not the τ ÷ (1 + m) selection uses, raised where the budget binds, and
      // reached a step at a time ({@link slewedTauPx}).
      const state = this.#views.get(view.id);
      if (state !== undefined) {
        state.targetTauPx = effectiveTauPx(view.bandTauPx, selection.limitExcess, view.weight);
        if (state.bandTauPx === null) {
          state.bandTauPx = state.targetTauPx;
          this.#bandsOf(state, view);
        }
      }
      return {
        id: view.id,
        cameraBodyFixedM: view.camera.positionM,
        orientationBodyFixed: view.camera.orientation,
        fovXRad: view.fovXRad,
        viewport: view.viewport,
        tauPx: view.tauPx,
        weight: view.weight,
        nearestM: Number.isFinite(nearestM) ? Math.max(nearestM, patchM) : nearestM,
      };
    });
    const demand = this.#withLookahead(selectionInput, selection, selectedViews, nowMs);
    const device = this.#device;
    // The same object every selection, rewritten in place; unseen forced patches are pinned by
    // `retain` but not drawn. The lookahead's patches are pinned as demanded alone: as they land,
    // until the next retain finds them neither selected nor beneath the selection.
    const resolved = device.resolver.resolve(selection);
    const drawSet = this.#present(
      device,
      demand === selection.demand ? selection : { ...selection, demand },
      resolved,
    );
    const grounded = input.grounded.map((c) => ({
      positionM: { ...c.positionM },
      radiusM: c.radiusM,
    }));
    const nearContacts = grounded.filter((contact) => {
      const reach = heldRadiusM(contact, patchM) + morphRampM(patchM);
      return drawSet.patches.some(
        ({ patch }) => distance(contact.positionM, patch.originM) <= patch.boundingRadiusM + reach,
      );
    });
    this.#selected = {
      selection,
      demand,
      drawSet,
      resolved,
      nearContacts,
      views: selectedViews,
      grounded,
      rangesVersion: this.#rangesVersion,
      surveyVersion: this.#survey?.version ?? 0,
    };
  }

  /**
   * `selection`'s demand joined by its lookahead's (`lookahead.ts`; plan R10, R10.T14.b), or its
   * own: with no lookahead clock, at the pass's first step, and where no view moves. The
   * lookahead selects by `selectPatches` over `input` with each moving view at its pose L ahead, at
   * half its weight; while a step's selection time over the latest steps is above 2 ms at p95, at
   * most every fourth selection, the last lookahead's demand asked in between.
   */
  #withLookahead(
    input: SelectionInput,
    selection: Selection,
    views: ReadonlyArray<SelectedView>,
    nowMs: number,
  ): ReadonlyArray<PatchRequest> {
    const steppedAtMs = this.#steppedAtMs;
    if (this.#lookaheadClockMs === null || steppedAtMs === null || !(nowMs > steppedAtMs)) {
      this.#aheadDemand = null;
      return selection.demand;
    }
    const perS = 1 / ((nowMs - steppedAtMs) / 1000);
    const motions = views.map((view, n): ViewMotion | null => {
      const was = this.#stepped.get(view.id);
      if (was === undefined) {
        return null;
      }
      const moved = sub(view.cameraBodyFixedM, was.positionM);
      return {
        velocityMps: vec3(moved.x * perS, moved.y * perS, moved.z * perS),
        // The move rule's distance: with no coarse patch in view, its own nearest surveyed ground.
        nearestM: Number.isFinite(view.nearestM)
          ? view.nearestM
          : (selection.nearestSurveyByViewM[n] ?? selection.nearestSurveyM),
      };
    });
    const aheadS = this.lookaheadS;
    const aheadInput = lookaheadInput(input, motions, aheadS);
    this.#sinceAhead += 1;
    if (aheadInput === null) {
      this.#aheadDemand = null;
      return selection.demand;
    }
    if (
      this.#aheadDemand === null ||
      this.#sinceAhead >= LOOKAHEAD_SLOW_EVERY ||
      !slowSelection(this.#stepCostsMs)
    ) {
      const startMs = performance.now();
      this.#aheadDemand = selectPatches(aheadInput).demand;
      this.#stepCostMs += performance.now() - startMs;
      this.#sinceAhead = 0;
    }
    // The views it looked ahead for: a still one would make every patch needed now.
    const cameras: MovingCamera[] = [];
    for (let n = 0; n < views.length; n += 1) {
      const view = views[n];
      const motion = motions[n] ?? null;
      if (view !== undefined && motion !== null && moving(motion)) {
        cameras.push({ positionM: view.cameraBodyFixedM, velocityMps: motion.velocityMps });
      }
    }
    const planet = this.#planet;
    return joinDemand(selection.demand, this.#aheadDemand, (key) =>
      timeToNeedS(planet, key, cameras, aheadS),
    );
  }

  /**
   * Notes when each key of `demand`, the patches asked for now that are not resident, was first
   * asked, for L's samples; a key no longer asked (`asked`, the rebakes with them) is forgotten,
   * and timed afresh if asked again.
   */
  #noteAsked(
    demand: ReadonlyArray<PatchRequest>,
    asked: ReadonlyMap<string, PatchRequest>,
    nowMs: number,
  ): void {
    const askedAtMs = this.#askedAtMs;
    for (const keyString of askedAtMs.keys()) {
      if (!asked.has(keyString)) {
        askedAtMs.delete(keyString);
      }
    }
    for (const request of demand) {
      const keyString = patchKeyString(request.key);
      if (!askedAtMs.has(keyString)) {
        askedAtMs.set(keyString, nowMs);
      }
    }
  }

  /**
   * The draw set of `resolved` as the time morph presents it (R10.T16.i), its finer patches held
   * through each coarsen, with the cache's pins and the coarse–fine holds made from it.
   */
  #present(device: Device, selection: Selection, resolved: DrawSet): DrawSet {
    // The hand-over holds the reference surface until every seen selected patch has a resident
    // patch to draw it, so that its first draw has no hole: the roots a view sees can land frames
    // apart, one of them drafted (the look-ahead's re-measure, 40.58 s on high at half-cost drafts).
    const held = !device.firstDrawn && resolved.missing > 0;
    // The wireframe's ground appears at once where none was drawn: its contours state elevations,
    // and a rise would draw ones the survey does not hold (the UX guide's principle 5).
    const drawSet = device.morph.present(
      held ? { ...resolved, count: 0 } : resolved,
      device.cache,
      this.#shading === "wire" ? NEVER_SEEN : this.#seenAtLastStep(selection),
    );
    device.firstDrawn ||= drawSet.count > 0;
    device.cache.retain(selection, drawSet);
    // The holds at the drawn set's coarse–fine edges, stand-ins included, the same for every view:
    // each view's bands are its own, and need not leave the coarse side at k = 0 there, nor the
    // fine side at 1 (decision-r10-t16-leans §9).
    const keys = device.drawnKeys;
    const levels = device.drawnLevels;
    keys.length = 0;
    levels.length = 0;
    for (const { patch, within } of drawSet.patches) {
      keys.push(within ?? patch.key);
      levels.push(patch.key.level);
    }
    device.holds.update(keys, levels);
    device.fadesAtMs = null;
    return drawSet;
  }

  /**
   * Whether a view saw an area at the last step (R10.T16.i): the bounds `selection` gives it, in
   * a view's frustum from its camera then. A patch drawn where nothing was rises out of the
   * reference surface over such ground, newly surveyed, and appears at once on ground newly in
   * view, which nothing showed.
   */
  #seenAtLastStep(selection: Selection): (area: PatchKey) => boolean {
    const views = this.#selected?.views ?? [];
    const frustums = views.flatMap((view) => {
      const camera = this.#stepped.get(view.id);
      return camera === undefined
        ? []
        : [
            {
              positionM: camera.positionM,
              frustum: frustumOf({
                orientation: camera.orientation,
                fovXRad: view.fovXRad,
                viewport: view.viewport,
              }),
            },
          ];
    });
    return (area) => {
      const bounds = selection.patches.get(patchKeyString(area))?.bounds;
      return (
        bounds !== undefined &&
        frustums.some(({ positionM, frustum }) =>
          inFrustum(relativeBounds(bounds, positionM), frustum),
        )
      );
    };
  }

  /**
   * How far a refine or a coarsen over the resident patch keyed `keyString` moves its ground, m: its
   * own morph's |h₀ − h₁|, or its disagreement with its resident parent's surface, which its heights
   * mix from (`parentDisagreementM`), where that is more; 0 for a patch not resident.
   */
  #morphSizeM(keyString: string): number {
    const residents = this.#device.residents;
    const resident = residents.get(keyString);
    const key = this.#device.cache.get(keyString)?.key;
    if (resident === undefined || key === undefined) {
      return 0;
    }
    const parent = parentKey(key);
    const above = parent === null ? undefined : residents.get(patchKeyString(parent));
    return above === undefined
      ? resident.sizes.morphM
      : Math.max(
          resident.sizes.morphM,
          parentDisagreementM(resident.sizes, above.sizes, parentQuadrant(key)),
        );
  }

  /**
   * The most pixels `sizeM` metres of ground over the resident patch keyed `keyString` subtend in a
   * view of the last step (R10.T16.i): seen square on from the view's camera at the patch's nearest
   * (its origin's distance less its bounding radius), a centre pixel's angle each; 0 for a patch
   * not resident.
   */
  #screenPx(keyString: string, sizeM: number): number {
    const patch = this.#device.cache.get(keyString);
    if (patch === undefined || !(sizeM > 0)) {
      return 0;
    }
    let px = 0;
    for (const view of this.#selected?.views ?? []) {
      const camera = this.#stepped.get(view.id);
      if (camera === undefined) {
        continue;
      }
      const nearestM = Math.max(
        1,
        norm(sub(patch.originM, camera.positionM)) - patch.boundingRadiusM,
      );
      const radPerPx = pixelAngleRad(
        { orientation: camera.orientation, fovXRad: view.fovXRad },
        view.viewport,
      );
      px = Math.max(px, sizeM / nearestM / radPerPx);
    }
    return px;
  }

  /** The drawn instances' fades at the draw time `nowMs`, made once a time ({@link Device.fades}). */
  #fadesAt(device: Device, nowMs: number): TransitionFades {
    if (device.fadesAtMs !== nowMs) {
      device.fades.update(device.drawnKeys, device.morph.footprintsAt(nowMs, this.#reducedMotion));
      device.fadesAtMs = nowMs;
      this.#releaseRetired(device);
    }
    return device.fades;
  }

  /** Frees the slots of the older bakes whose fades have ended (R10.T16.i). */
  #releaseRetired(device: Device): void {
    for (const slot of device.morph.takeReleased()) {
      device.cache.releaseRetired(slot);
    }
  }

  /**
   * The records written on the current device, or no draw while the device is lost (`refused`): a
   * view's frame buffers are made at its first frame, and nothing can be made then
   * (`EngineUnavailable`).
   */
  #writeOrNone(
    state: ViewState,
    mask: number,
    contacts: ReadonlyArray<GroundContact>,
    input: TerrainViewFrameInput,
    cascades: CascadeSet | null,
  ): {
    readonly draws: ReadonlyArray<DrawItem>;
    readonly patches: TerrainDrawnPatches;
    readonly count: number;
    readonly refused: boolean;
    readonly cast: CasterFrame;
    readonly overlayDepth: TerrainOverlayDepth | null;
    readonly overlayPass: number | null;
    readonly lit: boolean;
  } {
    try {
      return {
        ...this.#write(this.#device, state, mask, contacts, input, cascades),
        refused: false,
      };
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return {
          draws: NO_DRAWS,
          patches: NO_PATCHES,
          count: 0,
          refused: true,
          cast: NO_CASTERS,
          overlayDepth: null,
          overlayPass: null,
          lit: false,
        };
      }
      throw error;
    }
  }

  /**
   * Writes view `state`'s records, the drawn patches in `mask`, relative to its camera, and returns
   * its draws, with how many patches it draws; and, where its lit draw receives `cascades`
   * (R10.T9.b), draws their caster passes, which it returns.
   *
   * @remarks
   * On a lit pass whose interior form is made for the frame's lighting, the interior patches'
   * records (plan R14, R14.T1.a) are written apart from the others', each in the draw set's order,
   * and drawn first, by the interior patches' mesh; without it every patch is drawn by the lit
   * mesh, as before the split. A stand-in is never an interior patch, so that neither the interior
   * mesh nor the cluster path (R14.T1.b) draws an instance its slot's cluster records may not bound:
   * seamless-1's sub-patch stand-ins, an ancestor's slot and a sub-rectangle of its grid, are
   * stand-ins, and take the edge path in every pass: plan R14's T2.a would mask their clusters only
   * where seamless-1 draws one as its ancestor's own blocks, which it never does (it draws a
   * sub-patch's own grid on the ancestor's triangles, `terrainPatch.wgsl`'s `sourceGridOf`).
   *
   * On the lit pass's cluster path (plan R14, R14.T2.a; `GeometrySettings.litClusters`), the
   * interior patches draw from the lit pass's buckets at stride 1 in their interior draw's place,
   * each cluster kept by the cull's camera frustum and the body's horizon, and written in the draw
   * set's order by the stable compaction; the edge patches' draw follows, as before.
   *
   * Where every pass of the frame draws them there (`GeometrySettings.finClusters`, plan R14,
   * R14.T1.g; `#finsDrawn`), the edge patches but the stand-ins join the interior patches' records
   * in the draw set's order, marked in {@link ViewState.interiorFins}: their opaque clusters draw
   * from the strided buckets, their may-discard ones (the survey's edge, their skirt heads and
   * fins) from the may-discard buckets by the fin forms, in the lit pass, the casters and the
   * overlay's depth, and the lit mesh draws only the stand-ins. Each record whose heights fade
   * (R10.T16.i) is marked in {@link ViewState.interiorFading}.
   */
  #write(
    device: Device,
    state: ViewState,
    mask: number,
    contacts: ReadonlyArray<GroundContact>,
    input: TerrainViewFrameInput,
    cascades: CascadeSet | null,
  ): {
    readonly draws: ReadonlyArray<DrawItem>;
    readonly patches: TerrainDrawnPatches;
    readonly count: number;
    readonly cast: CasterFrame;
    readonly overlayDepth: TerrainOverlayDepth | null;
    readonly overlayPass: number | null;
    readonly lit: boolean;
  } {
    const { resources, residents } = device;
    const { instances, morph, phases, interiorPhases, drawn: drawnIndices, interior } = state;
    const { cameraM } = input.view;
    const [r0, r1, r2] = input.view.rotation.rows;
    const drawSet = this.#selected?.drawSet;
    const wire = input.wire;
    const { holds } = device.holds;
    const lit = this.#shading === "lit";
    const forms = lit ? this.#litForms(device, input) : NO_FORMS;
    const wireSplit = this.#wireSplits(device, state);
    const split = (this.#geometry.clusterPath && forms.interior !== null) || wireSplit;
    const fades = this.#fadesAt(device, input.nowMs);
    // The edge patches join the interior patches' records only where every pass draws them there.
    const fins = lit && split && this.#finsDrawn(device, state, forms);
    instances.clear();
    interior.clear();
    phases?.clear();
    interiorPhases?.clear();
    drawnIndices.length = 0;
    state.interiorDrawn.length = 0;
    state.interiorFins.length = 0;
    state.interiorFading.length = 0;
    let finCount = 0;
    if (drawSet !== undefined) {
      // `patches` holds exactly the `count` drawn patches (`DrawSetResolver`), and `holds` their
      // morph holds in the same order (`#select`).
      for (let n = 0; n < drawSet.count; n += 1) {
        const drawn = drawSet.patches[n];
        if (drawn === undefined || ((drawSet.views[n] ?? 0) & mask) === 0) {
          continue;
        }
        // R · origin − camera, component by component, so that a frame makes no vector a patch.
        const { patch, within } = drawn;
        const o = patch.originM;
        const level = patch.key.level;
        // A stand-in, whole or seamless-1's sub-patch of its ancestor's slot, draws by the edge path
        // in every pass: the slot's cluster records bound its whole patch alone (plan R14). An edge
        // patch joins the interior patches where `fins` draws it by its records (R14.T1.g).
        const resident = residents.get(drawn.keyString);
        const interiorBake = resident?.interior === true;
        const inner = split && !drawn.standIn && (interiorBake || (fins && resident !== undefined));
        const heights = device.morph.heightWords[n] ?? 0;
        if (inner) {
          state.interiorFins.push(!interiorBake);
          state.interiorFading.push(((heights >>> 8) & MIX_STEPS) !== 0);
          finCount += interiorBake ? 0 : 1;
        }
        (inner ? interior : instances).pushXyz(
          patch.slot,
          r0.x * o.x + r0.y * o.y + r0.z * o.z - cameraM.x,
          r1.x * o.x + r1.y * o.y + r1.z * o.z - cameraM.y,
          r2.x * o.x + r2.y * o.y + r2.z * o.z - cameraM.z,
          morph[2 * level] ?? 0,
          morph[2 * level + 1] ?? 0,
          this.#parentSlotOf(device, patch.key),
          parentQuadrant(patch.key),
          holds[n] ?? 0,
          within === null ? 0 : subPatchOf(patch.key, within),
          fades.edges[n] ?? 0,
          fades.corners[n] ?? 0,
          fadeWordOf(fades.own[n] ?? 0, heights),
        );
        (inner ? state.interiorDrawn : drawnIndices).push(n);
        const phasesOf = inner ? interiorPhases : phases;
        if (phasesOf !== null && wire !== undefined) {
          this.#pushPhase(device, phasesOf, drawn.keyString, patch.originM, wire);
        }
      }
    }
    const records = state.contacts;
    records.clear();
    const patchM = finestPatchSizeM(this.#planet);
    for (const contact of contacts) {
      const c = contact.positionM;
      records.pushXyz(
        r0.x * c.x + r0.y * c.y + r0.z * c.z - cameraM.x,
        r1.x * c.x + r1.y * c.y + r1.z * c.z - cameraM.y,
        r2.x * c.x + r2.y * c.y + r2.z * c.z - cameraM.z,
        heldRadiusM(contact, patchM),
        morphRampM(patchM),
      );
    }
    resources.writeFrame(instances, records, state.id);
    if (lit || wireSplit) {
      resources.writeInterior(interior, state.id);
    }
    if (phases !== null) {
      resources.writePhases(phases, state.id);
    }
    if (wireSplit && interiorPhases !== null) {
      resources.writePhases(interiorPhases, state.id, "interior");
    }
    const count = instances.count + interior.count;
    if (device.material === null || count === 0) {
      return {
        draws: NO_DRAWS,
        patches: NO_PATCHES,
        count,
        cast: NO_CASTERS,
        overlayDepth: null,
        overlayPass: null,
        lit: false,
      };
    }
    rotationMat4(input.view.rotation, state.uniforms.bodyRotation);
    let draws: ReadonlyArray<DrawItem>;
    let live: CascadeSet | null = null;
    // Whether the wireframe's interior patches drew through the cluster path (plan R14, R14.T4.c).
    let wireLit = false;
    switch (this.#shading) {
      case "lit": {
        const made = this.#writeLit(device, state, input, cascades, forms);
        draws = made.draws;
        live = made.live;
        break;
      }
      case "wire": {
        // A wireframe view's frame with no contours to draw draws nothing (R10.T11).
        if (wire === undefined) {
          draws = NO_DRAWS;
          break;
        }
        const edgeDraw = this.#writeWire(device, state, device.material, wire);
        if (!wireSplit || interior.count === 0) {
          draws = [edgeDraw];
          break;
        }
        // The interior patches through the cull and the stable compaction at stride 1, before the
        // edge patches' draw (plan R14, R14.T4.c; Design note 13).
        const wireClusters = this.#wireClusterPath(device, state, edgeDraw);
        this.#cullClusters(device, state, wireClusters, input, null);
        const wireBase = wireClusters.litBase;
        if (wireBase === null) {
          throw new Error("the wireframe's cluster path has no draw base");
        }
        draws = [
          ...wireClusters.buckets.drawItems(CLUSTER_LIT_PASS, wireClusters.mesh, wireBase, 1),
          ...(instances.count > 0 ? [edgeDraw] : []),
        ];
        wireLit = true;
        break;
      }
      case "spike":
        draws = [this.#writeSpike(device, state, device.material, input)];
        break;
    }
    const clusters = draws.length > 0 && lit ? this.#clusterPath(device, state, forms) : null;
    const litBase = clusters?.litBase ?? null;
    if (finCount > 0 && litBase === null && draws.length > 0) {
      // `#finsDrawn` holds every part the lit pass's cluster path needs: none is missing.
      throw new Error(
        "a lit view's edge patches joined the cluster path, which does not draw them",
      );
    }
    // The edge patches' draws: every draw but the interior patches'.
    const edge = draws.filter((draw) => draw.mesh !== resources.interiorMesh);
    const finLitBase = clusters?.finLitBase ?? null;
    const drawn =
      clusters !== null && litBase !== null
        ? [
            ...clusters.buckets.drawItems(CLUSTER_LIT_PASS, clusters.mesh, litBase, 1, finLitBase),
            ...edge,
          ]
        : draws;
    const cast =
      draws.length > 0 && live !== null
        ? this.#drawCasters(device, state, input, live, clusters)
        : NO_CASTERS;
    // Without casters the overlay's and the lit pass's passes are culled alone (the low setting's,
    // and w = 0's).
    const alone = clusters !== null && (clusters.overlay || litBase !== null);
    const culled = cast !== NO_CASTERS && clusters !== null && (cast.clustered || alone);
    if (clusters !== null && alone && !culled) {
      this.#cullClusters(device, state, clusters, input, null);
    }
    // The overlay's depth: on the cluster path its own pass's clusters, and by the redraw with the
    // lit pass's, the interior patches' drawn triangles with their own depth (plan R14, R14.T2.a).
    const overlayBase = clusters?.overlayBase ?? null;
    const depthBase = clusters?.depthBase ?? null;
    const finDepthBase = clusters?.finDepthBase ?? null;
    let overlayDepth: TerrainOverlayDepth | null = null;
    let overlayPass: number | null = null;
    if (clusters !== null && overlayBase !== null) {
      overlayPass = CLUSTER_OVERLAY_PASS;
      overlayDepth = {
        edge,
        clusters: clusters.buckets.drawItems(
          CLUSTER_OVERLAY_PASS,
          clusters.mesh,
          overlayBase,
          1,
          finDepthBase,
        ),
      };
    } else if (clusters !== null && litBase !== null && depthBase !== null) {
      overlayPass = CLUSTER_LIT_PASS;
      overlayDepth = {
        edge,
        clusters: clusters.buckets.drawItems(
          CLUSTER_LIT_PASS,
          clusters.mesh,
          depthBase,
          1,
          finDepthBase,
        ),
      };
    }
    const patches =
      draws.length > 0
        ? {
            interior: interior.count - finCount,
            edge: instances.count + finCount,
            fins: finCount,
          }
        : NO_PATCHES;
    return {
      draws: drawn,
      patches,
      count,
      cast,
      overlayDepth,
      overlayPass,
      lit: litBase !== null || wireLit,
    };
  }

  /**
   * Whether view `state`'s wireframe frame on `device` draws its interior patches through the
   * cluster path (plan R14, R14.T4.c): its geometry asks it, and its form, both kernels, the cluster
   * mesh and records and the view's interior buffers are made, so that every interior record the
   * frame writes apart is drawn.
   */
  #wireSplits(device: Device, state: ViewState): boolean {
    const { resources } = device;
    const view = resources.viewBuffers(state.id);
    return (
      this.#shading === "wire" &&
      this.#geometry.clusterPath &&
      this.#geometry.wireClusters &&
      device.wireClusterMaterial !== null &&
      device.cullKernel !== null &&
      device.compactKernel !== null &&
      resources.clusterMesh !== null &&
      resources.clusterRecords !== null &&
      view.interior !== null &&
      view.interiorPhases !== null
    );
  }

  /**
   * View `state`'s wireframe interior patches' cluster path (plan R14, R14.T4.c): its buckets, the
   * lit pass's ordered camera pass alone, and the bucket draws' shared part, the wireframe draw
   * `beside`'s but its mesh, indirect arguments and material, with the interior records, their
   * phases and the lists; each interior record's cull input the lit pass's bit.
   *
   * @throws Error where the frame splits its interior patches ({@link TerrainPass}'s `#wireSplits`)
   *   but a part is missing.
   */
  #wireClusterPath(device: Device, state: ViewState, beside: DrawItem): ClusterPath {
    const { resources } = device;
    const view = resources.viewBuffers(state.id);
    const buffers = resources.clusterBuffers(state.id);
    const mesh = resources.clusterMesh;
    const kernel = device.cullKernel;
    const material = device.wireClusterMaterial;
    const { interior, interiorPhases } = view;
    if (
      buffers === null ||
      mesh === null ||
      kernel === null ||
      material === null ||
      interior === null ||
      interiorPhases === null
    ) {
      throw new Error("a wireframe view's interior patches have no cluster path");
    }
    const slots = resources.layout.slots.slotCount;
    const clusters = (state.clusters ??= {
      inputs: new Uint32Array(2 * slots),
      passes: new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES),
      params: new ArrayBuffer(CULL_PARAMS_BYTES),
      casterBase: null,
      overlayBase: null,
      litBase: null,
      litBeside: null,
      finLitBase: null,
      finCasterBase: null,
      finDepthBase: null,
      device: null,
      stats: null,
    });
    if (clusters.device !== device) {
      clusters.litBase = null;
      clusters.litBeside = null;
      clusters.device = device;
    }
    if (clusters.litBeside !== beside || clusters.litBase?.material !== material) {
      const { mesh: _mesh, indirect: _indirect, instanceCount: _count, ...shared } = beside;
      clusters.litBase = {
        ...shared,
        material,
        storageBuffers: {
          ...beside.storageBuffers,
          instances: interior.instances,
          wirePhases: interiorPhases,
          clusterLists: buffers.buckets.lists,
        },
      };
      clusters.litBeside = beside;
    }
    const radiusM = this.#planet.figure.equatorialRadiusM;
    const drawSet = this.#selected?.drawSet;
    for (let record = 0; record < state.interiorDrawn.length; record += 1) {
      const drawnIndex = state.interiorDrawn[record];
      const patch = drawnIndex === undefined ? undefined : drawSet?.patches[drawnIndex]?.patch;
      // A record whose patch the draw set no longer holds enters no pass.
      clusters.inputs[2 * record] = patch === undefined ? 0 : LIT_PASS_BIT;
      clusters.inputs[2 * record + 1] = f32Bits(
        patch === undefined ? 0 : vertexSpacing(radiusM, patch.key.level).maxM,
      );
    }
    return {
      buckets: buffers.buckets,
      cullInputs: buffers.cullInputs,
      inputs: clusters.inputs,
      mesh,
      casterBase: null,
      overlayBase: null,
      depthBase: null,
      overlay: false,
      kernel,
      litBase: clusters.litBase,
      marks: buffers.marks,
      tallies: buffers.tallies,
      compact: device.compactKernel,
      finLitBase: null,
      finCasterBase: null,
      finDepthBase: null,
    };
  }

  /**
   * The lit materials a frame of `input`'s lighting draws with on `device`: the ground's emission's
   * form where the lighting carries it and it is made, the plain form until then (R08.T18.a), and
   * beside it the interior patches' form of the same (plan R14, R14.T1.a), or `null` where that is
   * not made, when the interior patches are drawn with the others. The first frame that carries the
   * emission asks for both emissive forms.
   */
  #litForms(device: Device, input: TerrainViewFrameInput): LitForms {
    const emission = input.lighting?.emission !== undefined;
    if (emission && !device.emissiveAsked) {
      device.emissiveAsked = true;
      const path = device.resources.layout.vertexPath;
      void this.#readyEmissive(device, path).catch((error: unknown) => {
        console.error("the terrain's emissive form could not be taken:", error);
      });
      void this.#readyInterior(device, "lit-emissive-interior", path).catch((error: unknown) => {
        console.error("the terrain's interior emissive form could not be taken:", error);
      });
      void this.#readyLitClusters(device, "lit-emissive-cluster", path).catch((error: unknown) => {
        console.error("the terrain's emissive cluster form could not be taken:", error);
      });
      void this.#readyFin(device, "lit-emissive-fin", path).catch((error: unknown) => {
        console.error("the terrain's emissive fin form could not be taken:", error);
      });
    }
    return emission && device.emissiveMaterial !== null
      ? {
          edge: device.emissiveMaterial,
          interior: device.emissiveInteriorMaterial,
          cluster: device.emissiveLitClusterMaterial,
          fin: device.emissiveLitFinMaterial,
        }
      : {
          edge: device.material,
          interior: device.interiorMaterial,
          cluster: device.litClusterMaterial,
          fin: device.litFinMaterial,
        };
  }

  /** R05's spike shading's uniforms for this frame, and its draw item, made once a device. */
  #writeSpike(
    device: Device,
    state: ViewState,
    material: MaterialHandle,
    input: TerrainViewFrameInput,
  ): DrawItem {
    const u = state.uniforms;
    const sun = input.sunDirectionBodyFixed;
    u.sunDirection[0] = sun.x;
    u.sunDirection[1] = sun.y;
    u.sunDirection[2] = sun.z;
    const k = (SPIKE_ALBEDO / Math.PI) * input.exposureScale;
    const [red, green, blue] = input.sunIlluminanceLx;
    u.sunRadiance[0] = red * k;
    u.sunRadiance[1] = green * k;
    u.sunRadiance[2] = blue * k;
    if (state.draw === null || state.drawDevice !== device) {
      state.draw = this.#drawItem(device, state, material);
      state.drawDevice = device;
    }
    return state.draw;
  }

  /**
   * The slot of `key`'s parent whose horizon map and normals the patch's blend to by its morph
   * factor (R10.T8.b, T8.c; decision-r10-t8a §1), or {@link NO_PARENT_SLOT} where there is none to
   * blend to: a root, a parent not resident, or one whose slot holds no map (on the lit layout
   * every bake holds its normals and its map, so the map stands for both). R05's streaming gate
   * splits only baked patches and `PatchCache.retain` pins the drawn patches' ancestors, so outside
   * a forced region the parent is nearly always resident; inside one the grounded hold keeps the
   * morph at 0.
   */
  #parentSlotOf(device: Device, key: PatchKey): number {
    if (key.level === 0) {
      return NO_PARENT_SLOT;
    }
    const parent = device.cache.residentOver(key, key.level - 1);
    return parent !== undefined && device.resources.holdsHorizon(parent.slot)
      ? parent.slot
      : NO_PARENT_SLOT;
  }

  /**
   * A lit frame's uniforms and lights, its shadow cascades' fields (R10.T9.b), and its draws, made
   * again where its tables or its bound shadow texture changed: the interior patches' (plan R14,
   * R14.T1.a), where it has any, then the others', where it has any; with the cascades its draws
   * receive, `null` where they receive none.
   *
   * @remarks
   * The draws receive `cascades` where their weight is above 0 and the caster material is made, the
   * set then made at its first need; they bind the set's layers while the set lives, and the
   * placeholder before. Otherwise their cascades' fields take w = 0, so that they sample nothing.
   * The interior patches' draw is the other draw's but its mesh, its material (`forms`'), its
   * instance records and its indirect arguments.
   */
  #writeLit(
    device: Device,
    state: ViewState,
    input: TerrainViewFrameInput,
    cascades: CascadeSet | null,
    forms: LitForms,
  ): { readonly draws: ReadonlyArray<DrawItem>; readonly live: CascadeSet | null } {
    const { resources } = device;
    const shadows = this.#shadows;
    if (shadows === null) {
      throw new Error("a lit terrain pass has no shadow cascades to bind");
    }
    const lighting = input.lighting;
    if (lighting !== undefined) {
      resources.setLighting(lighting, state.id);
    }
    const camera = rotateToBodyFixed(input.view.rotation, input.view.cameraM);
    const c = state.litUniforms.cameraBodyFixedM;
    c[0] = camera.x;
    c[1] = camera.y;
    c[2] = camera.z;
    c[3] = 1 / this.#planet.figure.equatorialRadiusM;
    const table = resources.phaseTable;
    const atmosphere = lighting?.atmosphere ?? device.airless;
    const material = forms.edge;
    const drawable = table !== null && atmosphere !== null && material !== null;
    const live =
      drawable &&
      cascades !== null &&
      cascades.weight > 0 &&
      device.shadowMaterial !== null &&
      shadows.cascadeTarget() !== null
        ? cascades
        : null;
    // The receivers sample each far cascade the frame holds as its own frame drew it (R14.T6.b).
    packCascadeUniforms(
      live === null ? null : (state.farPlan?.received ?? live),
      viewAxisOf(input.view.orientation),
      state.litUniforms,
    );
    if (table === null || atmosphere === null || material === null) {
      return { draws: NO_DRAWS, live: null };
    }
    const shadow = shadows.bound();
    if (
      state.draw === null ||
      state.drawDevice !== device ||
      state.drawnTable !== table ||
      state.drawnAtmosphere !== atmosphere ||
      state.drawnShadow !== shadow ||
      state.draw.material !== material
    ) {
      state.draw = this.#drawItem(device, state, material, { table, atmosphere, shadow });
      state.drawDevice = device;
      state.drawnTable = table;
      state.drawnAtmosphere = atmosphere;
      state.drawnShadow = shadow;
    }
    const draw = state.draw;
    const interior = state.interior.count > 0 ? this.#interiorDrawOf(device, state, forms) : null;
    const draws =
      interior === null ? [draw] : state.instances.count > 0 ? [interior, draw] : [interior];
    return { draws, live };
  }

  /**
   * View `state`'s interior patches' draw (plan R14, R14.T1.a): its lit draw with the interior
   * patches' mesh, their material (`forms`' interior), their instance records and their indirect
   * arguments, made again where the lit draw or the material changed.
   *
   * @throws Error where the frame has interior records but no interior form, or the layout no
   *   interior buffers: {@link TerrainPass}'s `#write` writes them apart only with both.
   */
  #interiorDrawOf(device: Device, state: ViewState, forms: LitForms): DrawItem {
    const beside = state.draw;
    const material = forms.interior;
    const mesh = device.resources.interiorMesh;
    const buffers = device.resources.viewBuffers(state.id).interior;
    if (beside === null || material === null || mesh === null || buffers === null) {
      throw new Error("a lit view's interior patches have no draw: its form, mesh or buffers");
    }
    if (
      state.interiorDraw === null ||
      state.interiorBeside !== beside ||
      state.interiorDraw.material !== material
    ) {
      state.interiorDraw = {
        ...beside,
        mesh,
        material,
        storageBuffers: { ...beside.storageBuffers, instances: buffers.instances },
        indirect: { buffer: buffers.indirect, offsetBytes: 0 },
      };
      state.interiorBeside = beside;
    }
    return state.interiorDraw;
  }

  /**
   * Draws view `state`'s casters into each of `set`'s cascades (plan R10, R10.T9.b;
   * decision-r10-t9b-design D4), one pass a cascade rendered, and returns the passes.
   *
   * @remarks
   * Each cascade's casters are the view's own records this frame whose patch's bounding sphere (the
   * resident bake's origin and bounding radius) meets the cascade's light-space box with the caster
   * reach ({@link castsInto}), so that a caster is drawn with the view's own morph bands, holds and
   * parents and stores the surface the view draws. The edge patches' records, and the interior
   * patches' off the cluster path, are copied as they stand into the view's caster buffer for the
   * cascade and drawn with a direct instance count, through the caster material, the lit mesh and
   * the view's storage and `Draw` values. Each pass, into the cascade's layer with its light
   * rotation and projection, draws the terrain's draws, where it has casters, and then the other
   * casters' ({@link TerrainShadows.casters}); every rendered pass is drawn, clearing its layer,
   * so that no other view's casts remain in it. The set renders them under the view's name, each a
   * submission of its own, before this returns.
   *
   * On the cluster path (plan R14, R14.T1.b; `GeometrySettings.clusterPath`), the interior
   * patches' records cast from the interior draw's own buffer: each takes, as its cull input, the
   * cascades `castsInto` admits its patch to and its level's largest vertex spacing; one dispatch of
   * the cull then tests each of its clusters against each such cascade's box at stride 1 and appends
   * it to that cascade's bucket, and each pass draws its buckets after the lit mesh's casters,
   * through the cluster caster. A layer's depth is its casters' nearest whatever their order, and a
   * cluster is refused only where it can write no texel of the layer, so the layers are those the
   * lit mesh draws.
   *
   * With `GeometrySettings.casterDepthCull` (plan R14, R14.T1.c; `clusters/receivers.ts`), a patch,
   * and on the cluster path each of its clusters, is also refused from a cascade where no receiver
   * that samples the cascade can read a texel it writes, by the view depths the receivers sample it
   * at and the next nearer cascade's cover. The layers then differ only where no receiver samples,
   * and the frame not at all. A stand-in casts by `castsInto` alone.
   *
   * With `GeometrySettings.casterStrideCap` above 1 (plan R14, R14.T1.d), each interior patch casts
   * into each cascade at the coarsest stride s, up to the cap, with s times its level's largest
   * vertex spacing within the cascade's texel, every cluster of the patch at the one stride, so
   * that no crack opens inside it; the skirts close the rest. The lit image moves by that bound.
   *
   * A far cascade the view's plan holds (plan R14, R14.T6.b; {@link FarCascadeHolds}) takes no
   * caster, no cull pass and no pass, and keeps its layer; the set records the view as each drawn
   * layer's owner, and the plan is committed. So does a cascade the view reuses (R14.T6.a;
   * {@link CascadeReuse}): every input of its casters as at its last render of the layer, which it
   * rendered last, on the cluster path.
   */
  #drawCasters(
    device: Device,
    state: ViewState,
    input: TerrainViewFrameInput,
    set: CascadeSet,
    clusters: ClusterPath | null,
  ): CasterFrame {
    const shadows = this.#shadows;
    const target = shadows?.target ?? null;
    const material = device.shadowMaterial;
    const drawSet = this.#selected?.drawSet;
    if (shadows === null || target === null || material === null || drawSet === undefined) {
      return NO_CASTERS;
    }
    const { resources } = device;
    const slots = resources.layout.slots.slotCount;
    const casters = (state.casters ??= {
      records: [
        new InstanceRecords(slots),
        new InstanceRecords(slots),
        new InstanceRecords(slots),
        new InstanceRecords(slots),
      ],
      items: null,
      device: null,
      material: null,
    });
    for (const records of casters.records) {
      records.clear();
    }
    // The interior patches cast through the cluster path where its caster is made.
    const clustered = clusters !== null && clusters.casterBase !== null;
    // A far cascade the frame holds casts nothing and is not drawn (plan R14, R14.T6.b).
    const plan = state.farPlan;
    const rendered = plan?.rendered ?? ALL_CASCADES_RENDERED;
    // Each patch's centre less the camera, body-fixed, in `f64`, as the fit placed the cascades.
    const camera = rotateToBodyFixed(input.view.rotation, input.view.cameraM);
    // T1.c's receivers' cull of each patch, in the same body-fixed frame: a stand-in's sub-patch is
    // left to `castsInto` alone, as its draw is left to the edge path.
    const viewAxis = rotateToBodyFixed(input.view.rotation, viewAxisOf(input.view.orientation));
    const receivers = this.#geometry.casterDepthCull
      ? cascadeCullPasses(set, [set.basis.x, set.basis.y, set.basis.z], viewAxis, 1, true)
      : null;
    for (const [records, drawn, interior] of [
      [state.instances, state.drawn, false],
      [state.interior, state.interiorDrawn, true],
    ] as const) {
      for (let record = 0; record < drawn.length; record += 1) {
        const drawnIndex = drawn[record];
        // A record whose patch the draw set no longer holds casts nothing.
        const entry = drawnIndex === undefined ? undefined : drawSet.patches[drawnIndex];
        const patch = entry?.patch;
        let mask = 0;
        if (patch !== undefined) {
          const o = patch.originM;
          const fromCamera = { x: o.x - camera.x, y: o.y - camera.y, z: o.z - camera.z };
          const culled = entry?.standIn === true ? null : receivers;
          for (const index of CASCADES) {
            if (
              rendered[index] &&
              castsInto(set, index, fromCamera, patch.boundingRadiusM) &&
              (culled === null ||
                !receiversRefuse(culled[index], viewAxis, fromCamera, patch.boundingRadiusM))
            ) {
              mask |= 1 << index;
            }
          }
        }
        if (interior && clustered) {
          clusters.inputs[2 * record] = ((clusters.inputs[2 * record] ?? 0) | mask) >>> 0;
          continue;
        }
        // The interior patches' records cast as the others' do: a layer's depth is its casters'
        // nearest, whatever their order.
        for (const index of CASCADES) {
          if ((mask & (1 << index)) !== 0) {
            casters.records[index].pushCopy(records, record);
          }
        }
      }
    }
    if (casters.items === null || casters.device !== device || casters.material !== material) {
      casters.items = this.#casterItems(device, state, material);
      casters.device = device;
      casters.material = material;
    }
    // A cascade whose casters' inputs are all as at the view's last render of its layer, which it
    // rendered last, keeps its layer (plan R14, R14.T6.a); on the cluster path, as every R14 lever.
    const revisions = shadows.casters.revisions();
    const reuse = state.casterInputs;
    const reused: [boolean, boolean, boolean, boolean] = [false, false, false, false];
    if (this.#geometry.clusterPath) {
      reuse.start();
      this.#appendSharedInputs(reuse, state, input, set, camera, viewAxis, revisions);
      const shared = reuse.shared();
      for (const index of CASCADES) {
        if (!rendered[index]) {
          continue;
        }
        reuse.start();
        reuse.word(shared);
        this.#appendCascadeCasters(reuse, device, state, index, clustered ? clusters : null);
        if (revisions !== null && shadows.holds(state.farHolds, index) && reuse.unchanged(index)) {
          reused[index] = true;
        } else {
          reuse.rendered(index);
        }
      }
    } else {
      // The way back renders every cascade it draws, and the next frame on the path renders it too.
      reuse.forget();
    }
    // A reused cascade, as a held one, takes no cull pass and no pass.
    const dueOf = (index: CascadeIndex): boolean => rendered[index] && !reused[index];
    const due: readonly [boolean, boolean, boolean, boolean] = [
      dueOf(0),
      dueOf(1),
      dueOf(2),
      dueOf(3),
    ];
    if (clustered && reused.includes(true)) {
      const kept = due.reduce((bits, on, index) => (on ? bits : bits & ~(1 << index)), ~0);
      for (let record = 0; record < state.interior.count; record += 1) {
        clusters.inputs[2 * record] = (clusters.inputs[2 * record] ?? 0) & kept;
      }
    }
    if (clusters !== null && (clustered || clusters.overlay || clusters.litBase !== null)) {
      this.#cullClusters(device, state, clusters, input, clustered ? set : null, due);
    }
    const items = casters.items;
    const casterBase = clustered ? clusters.casterBase : null;
    const passes = CASCADES.filter((index) => due[index]).map((index): DepthLayerPass => {
      const records = casters.records[index];
      resources.writeCasters(state.id, index, records);
      const cascade = set.cascades[index];
      const others = shadows.casters.draws(set, index, state.id);
      const count = records.count;
      const terrain: DrawItem[] = count > 0 ? [{ ...items[index], instanceCount: count }] : [];
      if (clusters !== null && casterBase !== null) {
        terrain.push(
          ...clusters.buckets.drawItems(
            index,
            clusters.mesh,
            casterBase,
            this.#geometry.casterStrideCap,
            clusters.finCasterBase,
          ),
        );
      }
      return {
        label: TERRAIN_SHADOW_PASS_LABEL,
        layer: index,
        viewRotation: cascade.viewRotation,
        projection: cascade.projection,
        draws: [...terrain, ...others],
      };
    });
    shadows.renderCasters(state.farHolds, state.id, passes);
    if (plan !== null) {
      state.farHolds.commit(plan);
    }
    const drawn = (index: CascadeIndex): number => (due[index] ? casters.records[index].count : 0);
    return { passes, litMeshCasters: [drawn(0), drawn(1), drawn(2), drawn(3)], clustered, reused };
  }

  /**
   * Appends the caster inputs every cascade of view `state`'s frame shares to `reuse` (plan R14,
   * R14.T6.a): every cascade's matrices, splits, reach and slack and the light basis (T1.c's cull
   * of each reads its nearer one's), the caster uniforms, the body's rotation and the camera's place
   * and view axis in `f64`, the geometry settings, the contacts, and the other casters (their
   * registry's version and each one's revision).
   */
  #appendSharedInputs(
    reuse: CascadeReuse,
    state: ViewState,
    input: TerrainViewFrameInput,
    set: CascadeSet,
    camera: Vec3,
    viewAxis: Vec3,
    revisions: ReadonlyArray<number> | null,
  ): void {
    for (const cascade of set.cascades) {
      reuse.f32(cascade.viewRotation);
      reuse.f32(cascade.projection);
      reuse.f64(cascade.nearM);
      reuse.f64(cascade.farM);
      reuse.f64(cascade.fadeStartM);
      reuse.f64(cascade.radiusM);
      reuse.f64(cascade.texelM);
      reuse.f64(cascade.depthNearM);
      reuse.f64(cascade.depthFarM);
      reuse.f64(cascade.holdSlackM);
    }
    reuse.xyz(set.basis.x);
    reuse.xyz(set.basis.y);
    reuse.xyz(set.basis.z);
    reuse.xyz(camera);
    reuse.xyz(viewAxis);
    const [r0, r1, r2] = input.view.rotation.rows;
    reuse.xyz(r0);
    reuse.xyz(r1);
    reuse.xyz(r2);
    const uniforms = state.casterUniforms;
    reuse.f32(uniforms.bodyRotation);
    reuse.f32(uniforms.cameraBodyFixedM);
    reuse.f32(uniforms.atlas);
    const geometry = this.#geometry;
    reuse.word(geometry.casterStrideCap);
    reuse.word(geometry.farCascadeRate);
    reuse.word(OVERLAY_DEPTH_WORD[geometry.overlayDepth]);
    reuse.word(geometry.clusterPath ? 1 : 0);
    reuse.word(geometry.casterDepthCull ? 1 : 0);
    reuse.word(geometry.litClusters ? 1 : 0);
    reuse.word(geometry.figureClusters ? 1 : 0);
    reuse.word(geometry.hullClusters ? 1 : 0);
    reuse.word(geometry.wireClusters ? 1 : 0);
    reuse.word(geometry.finClusters ? 1 : 0);
    reuse.f64(geometry.pageBytes);
    reuse.f64(geometry.meshTolerancePx.camera);
    reuse.f64(geometry.meshTolerancePx.wireframeOccluder);
    reuse.bytes(state.contacts.bytes());
    reuse.word(this.#shadows?.casters.version ?? 0);
    for (const revision of revisions ?? []) {
      reuse.f64(revision);
    }
  }

  /**
   * Appends cascade `index`'s casters this frame to `reuse` (plan R14, R14.T6.a): each of its
   * lit-mesh caster records ({@link appendCasterRecord}), and on `clusters`' path each interior
   * record its cull takes, by its index, with its cull input; each list after its count.
   */
  #appendCascadeCasters(
    reuse: CascadeReuse,
    device: Device,
    state: ViewState,
    index: CascadeIndex,
    clusters: ClusterPath | null,
  ): void {
    const versions = device.slotVersions;
    const records = state.casters?.records[index];
    reuse.word(records?.count ?? 0);
    if (records !== undefined) {
      const words = records.words;
      for (let record = 0; record < records.count; record += 1) {
        appendCasterRecord(reuse, words, record, versions);
      }
    }
    if (clusters === null) {
      return;
    }
    const interior = state.interior;
    const words = interior.words;
    const bit = 1 << index;
    reuse.word(interior.count);
    for (let record = 0; record < interior.count; record += 1) {
      if (((clusters.inputs[2 * record] ?? 0) & bit) !== 0) {
        reuse.word(record);
        appendCasterRecord(reuse, words, record, versions);
        reuse.word(clusters.inputs[2 * record + 1] ?? 0);
      }
    }
  }

  /**
   * View `state`'s cluster path this frame (plan R14, R14.T1.b and T1.e): its buckets, cull inputs
   * and their staging, the cluster mesh, the bucket draws' shared parts of the casters, where the
   * cluster caster is made, and of the overlay's depth, where the cluster depth is made and the
   * overlay takes its depth from the cluster path, and the cull's kernel; `null` where the lever is
   * off, the kernel not made, the layout holds no clusters or the frame has no interior records,
   * when the interior patches cast by the lit mesh and the overlay redraws every draw. Each
   * interior record's cull input is its level's largest vertex spacing, its passes none until the
   * casters' or the overlay's mark them. An edge patch's record on the path (plan R14, R14.T1.g)
   * takes {@link FIN_ERROR_UNIT_M} as its unit, so that it casts at stride 1, and a record whose
   * heights fade (R10.T16.i) `CULL_INPUT_WHOLE`, so that the cull keeps its every cluster; where
   * the frame holds an edge patch, the may-discard buckets' draws' shared parts come too.
   */
  #clusterPath(device: Device, state: ViewState, forms: LitForms): ClusterPath | null {
    const { resources } = device;
    const kernel = device.cullKernel;
    const mesh = resources.clusterMesh;
    const interior = resources.viewBuffers(state.id).interior;
    if (
      !this.#geometry.clusterPath ||
      kernel === null ||
      mesh === null ||
      interior === null ||
      state.interior.count === 0
    ) {
      return null;
    }
    const buffers = resources.clusterBuffers(state.id);
    const heights = resources.field("heights");
    const surveyMask = resources.field("survey-mask");
    if (buffers === null || heights === null || surveyMask === null) {
      return null;
    }
    const slots = resources.layout.slots.slotCount;
    const clusters = (state.clusters ??= {
      inputs: new Uint32Array(2 * slots),
      passes: new ArrayBuffer(MAX_CULL_PASSES * CULL_PASS_BYTES),
      params: new ArrayBuffer(CULL_PARAMS_BYTES),
      casterBase: null,
      overlayBase: null,
      litBase: null,
      litBeside: null,
      finLitBase: null,
      finCasterBase: null,
      finDepthBase: null,
      device: null,
      stats: null,
    });
    if (clusters.device !== device) {
      clusters.casterBase = null;
      clusters.overlayBase = null;
      clusters.litBase = null;
      clusters.litBeside = null;
      clusters.finLitBase = null;
      clusters.finCasterBase = null;
      clusters.finDepthBase = null;
      clusters.device = device;
    }
    const offsets = resources.field("offsets");
    const storageBuffers: Readonly<Record<string, BufferHandle>> = {
      heights,
      slots: resources.slotRecords,
      instances: interior.instances,
      contacts: resources.viewBuffers(state.id).contacts,
      surveyMask,
      clusterLists: buffers.buckets.lists,
      ...(offsets === null ? {} : { offsets }),
    };
    const baseOf = (
      material: MaterialHandle | null,
      held: BucketDrawBase | null,
    ): BucketDrawBase | null =>
      material === null
        ? null
        : held?.material === material
          ? held
          : {
              material,
              offsetFromCameraM: ZERO_OFFSET,
              uniforms: state.casterUniforms,
              textures: {},
              storageBuffers,
            };
    clusters.casterBase = baseOf(device.clusterShadowMaterial, clusters.casterBase);
    clusters.overlayBase = baseOf(device.clusterDepthMaterial, clusters.overlayBase);
    clusters.finCasterBase = baseOf(device.finShadowMaterial, clusters.finCasterBase);
    clusters.finDepthBase = baseOf(device.finDepthMaterial, clusters.finDepthBase);
    const overlay =
      this.#geometry.overlayDepth !== "redraw" &&
      clusters.overlayBase !== null &&
      device.depthMaterial !== null;
    // The lit pass on the cluster path (plan R14, R14.T2.a): its form, the compaction and the cluster
    // depth made, and the interior draw it stands in for, of this frame's records.
    const beside = state.interiorDraw;
    const litForm = forms.cluster;
    const lit =
      this.#geometry.litClusters &&
      litForm !== null &&
      device.compactKernel !== null &&
      clusters.overlayBase !== null &&
      beside !== null &&
      beside.storageBuffers?.["instances"] === interior.instances;
    if (lit && (clusters.litBeside !== beside || clusters.litBase?.material !== litForm)) {
      const { mesh: _mesh, indirect: _indirect, instanceCount: _count, ...shared } = beside;
      clusters.litBase = {
        ...shared,
        material: litForm,
        storageBuffers: { ...beside.storageBuffers, clusterLists: buffers.buckets.lists },
      };
      clusters.litBeside = beside;
    }
    // The edge patches' may-discard buckets draw by the lit base with the fin form (R14.T1.g).
    const litBase = clusters.litBase;
    const finForm = forms.fin;
    if (
      litBase !== null &&
      finForm !== null &&
      (clusters.finLitBase?.material !== finForm ||
        clusters.finLitBase.storageBuffers !== litBase.storageBuffers)
    ) {
      clusters.finLitBase = { ...litBase, material: finForm };
    }
    const radiusM = this.#planet.figure.equatorialRadiusM;
    const drawSet = this.#selected?.drawSet;
    let fins = false;
    for (let record = 0; record < state.interiorDrawn.length; record += 1) {
      const drawnIndex = state.interiorDrawn[record];
      const patch = drawnIndex === undefined ? undefined : drawSet?.patches[drawnIndex]?.patch;
      // An edge patch casts at stride 1, as by its lit mesh (R14.T1.g).
      const fin = state.interiorFins[record] === true;
      fins ||= fin;
      // A record whose patch the draw set no longer holds enters no pass; one whose heights fade
      // is kept whole by each it enters (R10.T16.i).
      const whole = state.interiorFading[record] === true ? CULL_INPUT_WHOLE : 0;
      clusters.inputs[2 * record] =
        patch === undefined
          ? 0
          : ((overlay ? OVERLAY_PASS_BIT : 0) | (lit ? LIT_PASS_BIT : 0) | whole) >>> 0;
      clusters.inputs[2 * record + 1] = f32Bits(
        patch === undefined
          ? 0
          : fin
            ? FIN_ERROR_UNIT_M
            : vertexSpacing(radiusM, patch.key.level).maxM,
      );
    }
    return {
      buckets: buffers.buckets,
      cullInputs: buffers.cullInputs,
      inputs: clusters.inputs,
      mesh,
      casterBase: clusters.casterBase,
      overlayBase: overlay ? clusters.overlayBase : null,
      depthBase: clusters.overlayBase,
      overlay,
      kernel,
      litBase: lit ? clusters.litBase : null,
      marks: buffers.marks,
      tallies: buffers.tallies,
      compact: device.compactKernel,
      finLitBase: lit && fins ? clusters.finLitBase : null,
      finCasterBase: fins ? clusters.finCasterBase : null,
      finDepthBase: fins ? clusters.finDepthBase : null,
    };
  }

  /**
   * Whether view `state`'s lit frame on `device` would draw its interior patches through the lit
   * pass's cluster path with `forms` (plan R14, R14.T2.a): what `#clusterPath` and `#writeLit` need
   * of the geometry, the forms, the kernels, the cluster depth, the mesh, the records and the view's
   * buffers, but this frame's records and draw.
   */
  #litClustersReady(device: Device, state: ViewState, forms: LitForms): boolean {
    const { resources } = device;
    return (
      this.#geometry.clusterPath &&
      this.#geometry.litClusters &&
      forms.interior !== null &&
      forms.cluster !== null &&
      device.cullKernel !== null &&
      device.compactKernel !== null &&
      device.clusterDepthMaterial !== null &&
      resources.clusterMesh !== null &&
      resources.clusterRecords !== null &&
      resources.interiorMesh !== null &&
      resources.viewBuffers(state.id).interior !== null &&
      resources.field("heights") !== null &&
      resources.field("survey-mask") !== null
    );
  }

  /**
   * Whether view `state`'s lit frame on `device` draws its edge patches through the cluster path
   * beside the interior patches (plan R14, R14.T1.g; `GeometrySettings.finClusters`): the lit pass's
   * interior patches draw through it ({@link TerrainPass}'s `#litClustersReady`), and every fin
   * form a pass of the frame draws by is made, the lit form of `forms`, the overlay's depth, and,
   * where the interior patches cast by clusters, the caster; so that each record the frame writes to
   * the path is drawn by every pass it enters, as its lit mesh would be. Else they draw by the lit
   * mesh.
   */
  #finsDrawn(device: Device, state: ViewState, forms: LitForms): boolean {
    return (
      this.#geometry.finClusters &&
      this.#litClustersReady(device, state, forms) &&
      forms.fin !== null &&
      device.finDepthMaterial !== null &&
      (device.clusterShadowMaterial === null || device.finShadowMaterial !== null)
    );
  }

  /**
   * Culls view `state`'s interior patches' clusters into its buckets (plan R14, R14.T1.b–e): writes
   * each record's cull input, as the casters and the overlay marked them, `set`'s cascades as the
   * cull's first four passes, up to `casterStrideCap` and with their receivers' cull where
   * `casterDepthCull` is on, and where the overlay takes its depth from the cluster path its camera's
   * frustum as the fifth, at stride 1, and the dispatch's parameters; clears the buckets and
   * dispatches the cull, a workgroup a record, its submission before the caster passes' and the
   * overlay's; and, behind the measurement switch, samples the buckets' counts.
   *
   * @param set - The cascades the frame casts into on the cluster path, or `null` for none.
   * @param due - The cascades of `set` the frame renders: by default those the view's plan does
   *   not hold (plan R14, R14.T6.b), and less those it reuses (R14.T6.a).
   */
  #cullClusters(
    device: Device,
    state: ViewState,
    clusters: ClusterPath,
    input: TerrainViewFrameInput,
    set: CascadeSet | null,
    due: ReadonlyArray<boolean> = state.farPlan?.rendered ?? ALL_CASCADES_RENDERED,
  ): void {
    const { resources } = device;
    const held = state.clusters;
    const interior = resources.viewBuffers(state.id).interior;
    const records = resources.clusterRecords;
    if (held === null || interior === null || records === null) {
      return;
    }
    const engine = this.#engine;
    const count = state.interior.count;
    const [r0, r1, r2] = input.view.rotation.rows;
    // Body-fixed into the non-rotating axes the instance origins are in: R v = (r₀·v, r₁·v, r₂·v).
    const turn = (v: Vec3): Vec3 => ({
      x: r0.x * v.x + r0.y * v.y + r0.z * v.z,
      y: r1.x * v.x + r1.y * v.y + r1.z * v.z,
      z: r2.x * v.x + r2.y * v.y + r2.z * v.z,
    });
    const viewAxis = viewAxisOf(input.view.orientation);
    const { casterStrideCap, casterDepthCull } = this.#geometry;
    // A far cascade the frame holds (plan R14, R14.T6.b), or one it reuses (R14.T6.a), takes none.
    const cascades: ReadonlyArray<CullPass | null> =
      set === null
        ? [null, null, null, null]
        : cascadeCullPasses(
            set,
            [turn(set.basis.x), turn(set.basis.y), turn(set.basis.z)],
            viewAxis,
            casterStrideCap,
            casterDepthCull,
          ).map((pass, index) => (due[index] === true ? pass : null));
    const passes: Array<CullPass | null> = [
      ...cascades,
      clusters.overlay ? this.#overlayCullPass(input) : null,
      clusters.litBase !== null ? this.#litCullPass(input) : null,
    ];
    while (passes.length > CASCADES.length && passes.at(-1) === null) {
      passes.pop();
    }
    const { buckets } = clusters;
    packCullPasses(
      passes,
      passes.map((_, index) => buckets.place(index)),
      held.passes,
    );
    packCullParams(
      {
        rotationColumns: [
          { x: r0.x, y: r1.x, z: r2.x },
          { x: r0.y, y: r1.y, z: r2.y },
          { x: r0.z, y: r1.z, z: r2.z },
        ],
        instances: count,
        instanceWords: INSTANCE_RECORD_BYTES / 4,
        clustersPerPage: TERRAIN_PAGE_CLUSTERS,
        passes: passes.length,
        templateOffsets: buckets.templateOffsets,
        templateCapacities: buckets.capacities,
        viewAxis,
        firstShape: CLUSTER_SHAPE_CODE["terrain-block"],
        discardTemplates: TERRAIN_DISCARD_TEMPLATES,
      },
      held.params,
    );
    engine.writeBuffer(clusters.cullInputs, 0, clusters.inputs.subarray(0, 2 * count));
    buckets.clear();
    engine.dispatch(
      clusters.kernel,
      {
        uniforms: {
          params: new Uint32Array(held.params),
          passes: new Uint32Array(held.passes),
        },
        buffers: {
          instance_words: interior.instances,
          cull_inputs: clusters.cullInputs,
          records,
          lists: buckets.lists,
          args: buckets.args,
          marks: clusters.marks,
          // No terrain record is a DAG's, so its records stand in for the LOD spheres, unread.
          lods: records,
        },
        sampled: {},
        storage: {},
      },
      [count, 1, 1],
      TERRAIN_CLUSTER_CULL_LABEL,
    );
    if (clusters.litBase !== null && clusters.compact !== null) {
      this.#compactLitPass(clusters, clusters.compact, count);
    }
    if (this.#measuring) {
      held.stats ??= new ClusterStats(engine, `terrain clusters (${state.id})`);
      held.stats.sample(buckets, TERRAIN_CLUSTER_MESH.templates, CLUSTER_PASS_NAMES);
    }
  }

  /**
   * Writes the lit pass's buckets from the cull's marks in the draw set's order (plan R14, R14.T2.a;
   * `clusters/compact.ts`): the count, the scan and the scatter, three dispatches over `count`
   * interior records, each submitted after the cull's and before the frame's draws.
   */
  #compactLitPass(clusters: ClusterPath, kernel: ComputeHandle, count: number): void {
    const { buckets } = clusters;
    const place = buckets.place(CLUSTER_LIT_PASS);
    for (const phase of COMPACT_PHASES) {
      this.#engine.dispatch(
        kernel,
        {
          uniforms: {
            params: packCompactParams({
              instances: count,
              clustersPerPage: TERRAIN_PAGE_CLUSTERS,
              templates: TERRAIN_CLUSTER_MESH.templates.length,
              phase,
              bucketBase: place.bucketBase,
              listBase: place.listBase,
              templateOffsets: buckets.templateOffsets,
            }),
          },
          buffers: {
            marks: clusters.marks,
            tallies: clusters.tallies,
            lists: buckets.lists,
            args: buckets.args,
          },
          sampled: {},
          storage: {},
        },
        compactWorkgroups(phase, count),
        TERRAIN_CLUSTER_COMPACT_LABEL,
      );
    }
  }

  /**
   * The lit pass's camera as an ordered pass of the cull (plan R14, R14.T2.a): R05's frustum of the
   * view's camera in the instances' frame, each side opened by {@link OVERLAY_CULL_MARGIN_RAD}, and
   * R05's horizon occluder seen from it (`horizonCone`), at stride 1, its kept clusters marked for
   * the stable compaction.
   */
  #litCullPass(input: TerrainViewFrameInput): CameraCullPass {
    const { view } = input;
    const frustum = frustumOf(
      { orientation: view.orientation, fovXRad: view.fovXRad, viewport: view.viewport },
      OVERLAY_CULL_MARGIN_RAD,
    );
    const { occluderRadiusM } = horizonCone(
      this.#planet,
      rotateToBodyFixed(view.rotation, view.cameraM),
    );
    return {
      kind: "camera",
      planes: frustum.planes,
      toleranceRad: 0,
      strideCap: 1,
      horizon: cullHorizonOf(view.cameraM, occluderRadiusM),
      order: "ordered",
    };
  }

  /**
   * The overlay's camera as a pass of the cull (plan R14, R14.T1.e): R05's frustum of the view's
   * camera in the instances' frame, each side opened by {@link OVERLAY_CULL_MARGIN_RAD}, at stride
   * 1.
   */
  #overlayCullPass(input: TerrainViewFrameInput): CameraCullPass {
    const { view } = input;
    const frustum = frustumOf(
      { orientation: view.orientation, fovXRad: view.fovXRad, viewport: view.viewport },
      OVERLAY_CULL_MARGIN_RAD,
    );
    return {
      kind: "camera",
      planes: frustum.planes,
      toleranceRad: 0,
      strideCap: 1,
      horizon: null,
      order: "appended",
    };
  }

  /**
   * View `state`'s caster draws of `device`'s resources with the caster `material`, one a
   * cascade, but their counts (R10.T9.b): the lit mesh, R05's vertex storage with the cascade's
   * caster records as its instances, the view's contacts and the survey mask, the lit `Draw`'s own
   * fields, and no texture.
   */
  #casterItems(
    device: Device,
    state: ViewState,
    material: MaterialHandle,
  ): readonly [DrawItem, DrawItem, DrawItem, DrawItem] {
    const { resources } = device;
    const buffers = resources.viewBuffers(state.id);
    const casters = resources.casterBuffers(state.id);
    const heights = resources.field("heights");
    const surveyMask = resources.field("survey-mask");
    if (heights === null || surveyMask === null) {
      throw new Error("a lit terrain's casters have no lit layout: its heights or survey masks");
    }
    const offsets = resources.field("offsets");
    const item = (instances: BufferHandle): DrawItem => ({
      mesh: resources.mesh,
      material,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: state.casterUniforms,
      textures: {},
      storageBuffers: {
        heights,
        slots: resources.slotRecords,
        instances,
        contacts: buffers.contacts,
        surveyMask,
        ...(offsets === null ? {} : { offsets }),
      },
    });
    return [item(casters[0]), item(casters[1]), item(casters[2]), item(casters[3])];
  }

  /**
   * Appends a drawn patch's wireframe phases (R10.T11): its contour phase at the view's interval
   * from its bake's origin height, its origin's height above the sea, and its place for the
   * graticule at the view's step, kept for the patch until the step changes.
   */
  #pushPhase(
    device: Device,
    phases: WirePhaseRecords,
    keyString: string,
    originM: Vec3,
    wire: TerrainWireFrameInput,
  ): void {
    const resident = device.residents.get(keyString);
    if (resident === undefined) {
      // The draw set is the device's cache's, made again at a restore, and a patch is cached
      // only once its upload has made its record.
      throw new Error(`the drawn patch ${keyString} has no resident record`);
    }
    const { originHeightM } = resident;
    const stepRad = wire.graticule?.stepRad ?? 0;
    let held = device.graticules.get(keyString);
    if (held?.stepRad !== stepRad) {
      held = { stepRad, patch: graticulePatch(originM, stepRad) };
      device.graticules.set(keyString, held);
    }
    phases.push(
      contourPhase(originHeightM, wire.intervalM),
      wire.seaLevelM === null ? 0 : originHeightM - wire.seaLevelM,
      held.patch,
    );
  }

  /**
   * A wireframe frame's uniforms (R10.T11; `terrainWire.wgsl`'s `Draw`): its interval, stroke
   * scale and coastline, the contours' widths at the scale, the graticule's step and widths, the
   * colours and the slope term; and its draw, made once a device.
   */
  #writeWire(
    device: Device,
    state: ViewState,
    material: MaterialHandle,
    wire: TerrainWireFrameInput,
  ): DrawItem {
    const u = state.wireUniforms;
    const scale = wire.strokeScale;
    u.contour[0] = wire.intervalM;
    u.contour[1] = scale;
    u.contour[2] = wire.seaLevelM === null ? 0 : 1;
    // 1 ÷ CI in f64, narrowed once: the shader multiplies by it, which WGSL rounds correctly.
    u.contour[3] = 1 / wire.intervalM;
    u.contourWidthsPx[0] = CONTOUR_PX.minor * scale;
    u.contourWidthsPx[1] = CONTOUR_PX.index * scale;
    u.contourWidthsPx[2] = CONTOUR_PX.coastline * scale;
    const graticule = wire.graticule;
    u.graticule[0] = graticule?.stepRad ?? 0;
    u.graticule[1] = graticule?.thinPx ?? 0;
    u.graticule[2] = graticule?.heavyPx ?? 0;
    u.graticule[3] = graticule?.casingPx ?? 0;
    u.muted.set(wire.colours.muted);
    u.text.set(wire.colours.text);
    u.casing.set(wire.colours.casing);
    u.occluderSlopePx[0] = wire.occluderSlopePx;
    if (state.draw === null || state.drawDevice !== device) {
      state.draw = this.#wireDrawItem(device, state, material);
      state.drawDevice = device;
    }
    return state.draw;
  }

  /**
   * View `state`'s wireframe draw of `device`'s resources (R10.T11): R05's vertex storage, the
   * survey mask and the view's phases, its `Draw` the view's wireframe uniforms.
   */
  #wireDrawItem(device: Device, state: ViewState, material: MaterialHandle): DrawItem {
    const { resources } = device;
    const buffers = resources.viewBuffers(state.id);
    const heights = resources.field("heights");
    const surveyMask = resources.field("survey-mask");
    if (heights === null || surveyMask === null || buffers.phases === null) {
      throw new Error(
        "a wireframe terrain's draw has no wireframe layout: its heights, masks or phases",
      );
    }
    const storageBuffers: Record<string, BufferHandle> = {
      heights,
      slots: resources.slotRecords,
      instances: buffers.instances,
      contacts: buffers.contacts,
      surveyMask,
      wirePhases: buffers.phases,
    };
    const offsets = resources.field("offsets");
    if (offsets !== null) {
      storageBuffers["offsets"] = offsets;
    }
    return {
      mesh: resources.mesh,
      material,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: state.wireUniforms,
      textures: {},
      storageBuffers,
      indirect: { buffer: buffers.indirect, offsetBytes: 0 },
    };
  }

  /**
   * View `state`'s draw of `device`'s resources with `material`, its uniforms the view's arrays and
   * its frame buffers the view's own; a lit terrain's binds its class weights, laws, the view's
   * lights, the `phase` table and atmosphere tables too.
   */
  #drawItem(
    device: Device,
    state: ViewState,
    material: MaterialHandle,
    phase: {
      readonly table: TextureHandle;
      readonly atmosphere: SurfaceLightingTables;
      /** The shadow cascades' layers, or the placeholder (R10.T9.b). */
      readonly shadow: TextureHandle;
    } | null = null,
  ): DrawItem {
    const { resources } = device;
    const buffers: TerrainViewBuffers = resources.viewBuffers(state.id);
    const heights = resources.field("heights");
    if (heights === null) {
      throw new Error("the terrain layout has no heights buffer");
    }
    const storageBuffers: Record<string, BufferHandle> = {
      heights,
      slots: resources.slotRecords,
      instances: buffers.instances,
      contacts: buffers.contacts,
    };
    const offsets = resources.field("offsets");
    if (offsets !== null) {
      storageBuffers["offsets"] = offsets;
    }
    const atlas = normalsAtlasOf(resources.layout);
    state.uniforms.atlas.set([
      atlas.columns,
      atlas.tilesPerLayer,
      atlas.tileTexels,
      atlas.samplesPerSide,
    ]);
    const classLayers = resources.field("class-layers");
    if (phase === null) {
      return {
        mesh: resources.mesh,
        material,
        offsetFromCameraM: ZERO_OFFSET,
        uniforms: state.uniforms,
        textures: { normals: resources.normals },
        storageBuffers,
        indirect: { buffer: buffers.indirect, offsetBytes: 0 },
      };
    }
    const horizonMap = resources.field("horizon-map");
    const surveyMask = resources.field("survey-mask");
    if (
      buffers.lights === null ||
      classLayers === null ||
      horizonMap === null ||
      surveyMask === null
    ) {
      throw new Error(
        "a lit terrain's draw has no lit layout: its class layers, horizon maps, survey masks or lights",
      );
    }
    storageBuffers["classLayers"] = classLayers;
    storageBuffers["terrainLights"] = buffers.lights;
    storageBuffers["horizonMap"] = horizonMap;
    storageBuffers["surveyMask"] = surveyMask;
    return {
      mesh: resources.mesh,
      material,
      offsetFromCameraM: ZERO_OFFSET,
      uniforms: state.litUniforms,
      textures: {
        normals: resources.normals,
        phaseFactorTable: phase.table,
        ...surfaceLightingDrawTextures(phase.atmosphere),
        [SHADOW_CASCADES_TEXTURE]: phase.shadow,
      },
      storageBuffers,
      indirect: { buffer: buffers.indirect, offsetBytes: 0 },
    };
  }
}

/** The edge patches' fin forms (plan R14, R14.T1.g), as the session's material kinds name them. */
type FinKind = "lit-fin" | "lit-emissive-fin" | "depth-fin" | "shadow-fin";

/** Every fin form, for a device's release. */
const FIN_KINDS: ReadonlyArray<FinKind> = [
  "lit-fin",
  "lit-emissive-fin",
  "depth-fin",
  "shadow-fin",
];

/** `device`'s fin form of `kind`, `null` before it is made. */
function finMaterialOf(device: Device, kind: FinKind): MaterialHandle | null {
  let material: MaterialHandle | null;
  switch (kind) {
    case "lit-fin":
      material = device.litFinMaterial;
      break;
    case "lit-emissive-fin":
      material = device.emissiveLitFinMaterial;
      break;
    case "depth-fin":
      material = device.finDepthMaterial;
      break;
    case "shadow-fin":
      material = device.finShadowMaterial;
      break;
  }
  return material;
}

/** Sets `device`'s fin form of `kind`. */
function setFinMaterial(device: Device, kind: FinKind, material: MaterialHandle | null): void {
  switch (kind) {
    case "lit-fin":
      device.litFinMaterial = material;
      break;
    case "lit-emissive-fin":
      device.emissiveLitFinMaterial = material;
      break;
    case "depth-fin":
      device.finDepthMaterial = material;
      break;
    case "shadow-fin":
      device.finShadowMaterial = material;
      break;
  }
}

const ZERO_OFFSET = new Float32Array(3);

/** The bits of `value` narrowed to `f32`, as a `u32`. */
function f32Bits(value: number): number {
  F32_SCRATCH[0] = value;
  return U32_OF_F32[0] ?? 0;
}

const F32_SCRATCH = new Float32Array(1);
const U32_OF_F32 = new Uint32Array(F32_SCRATCH.buffer);

/** The camera's forward, its −z, in the axes its orientation is given in: the view axis. */
function viewAxisOf(orientation: Quaternion): Vec3 {
  const [r0, r1, r2] = rotationRows(orientation);
  return { x: -r0.z, y: -r1.z, z: -r2.z };
}

/** A selection of nothing: what a faulted pass draws. */
const EMPTY_SELECTION: Selection = {
  patches: new Map(),
  demand: [],
  limited: false,
  limitExcess: 0,
  hiddenBaked: [],
  nearestSurveyM: Number.POSITIVE_INFINITY,
  nearestSurveyByViewM: [],
};
