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
import { EngineUnavailable } from "../engine/resilientEngine";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import type {
  DepthLayerPass,
  DrawItem,
  MaterialHandle,
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
  type QualitySetting,
  TERRAIN_SETTINGS,
  type TerrainSettings,
} from "../quality/qualitySetting";
import { norm, type Vec3 } from "../../geometry/vec3";
import {
  TERRAIN_NOT_AVAILABLE,
  type TerrainAnnunciation,
  TerrainAnnunciationDebounce,
  type TerrainConditions,
  type TerrainFault,
} from "./annunciation";
import { distanceToBoxM, relativeBounds } from "./bounds";
import { type DrawSet, DrawSetResolver, PatchCache } from "./cache";
import { CoarseFineHolds } from "./coarseFineHolds";
import { vertexSpacing } from "./cube";
import { TERRAIN_SHADOW_PASS_LABEL, terrainMaterialSpec } from "./gpu/material";
import {
  DEFAULT_TERRAIN_VIEW,
  normalsAtlasOf,
  surveyCellCode,
  type TerrainLayout,
  TerrainResources,
  type TerrainViewBuffers,
} from "./gpu/resources";
import {
  ContactRecords,
  InstanceRecords,
  NO_PARENT_SLOT,
  parentQuadrant,
  WirePhaseRecords,
} from "./gpu/uniforms";
import { MAX_LEVEL, type PatchKey, patchKeyString } from "./patchKey";
import { type PlanetGeometry, planetGeometry } from "./planet";
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
  type SurveyLookup,
  type ViewSelectionInput,
} from "./select";
import { RESELECT_MOVE_FRACTION, selectionTolerancePx } from "./selectionTolerance";
import { TERRAIN_SHADING_TRAITS, type TerrainShading } from "./slotLayout";
import { ResidentHeights, type ResidentHeightsView } from "./residentHeights";
import { type TerrainMaterialSource, terrainMaterialOf } from "./materialSet";
import {
  type CascadeIndex,
  cascadeUniforms,
  type CascadeUniforms,
  packCascadeUniforms,
} from "./shadows/cascadeShadow";
import { type CascadeSet, discTopDirection, fitCascades } from "./shadows/cascades";
import { TerrainShadows } from "./shadows/terrainShadows";
import { SHADOW_CASCADES_TEXTURE, type TerrainLighting } from "./terrainMaterial";
import { CONTOUR_PX, contourPhase, type GraticulePatch, graticulePatch } from "./terrainWire";
import type {
  BakedPatch,
  BakeSettings,
  FieldBlockAnswer,
  HeightSourceSetting,
} from "./workers/messages";
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
  /** The draw list's slope term, device px (`WireframeDrawList.occluderSlopePx`). */
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
  /** The presented size in device pixels. */
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
   * The caster passes rendered into the set this frame, one a cascade, nearest first, each the
   * terrain's draw of the view's records in that cascade (where it has any) and then the other
   * casters' ({@link TerrainShadows.casters}); none at w = 0, before the caster material is
   * compiled, or with no draw of the view's terrain, when the lit draw samples nothing.
   */
  readonly passes: ReadonlyArray<DepthLayerPass>;
}

/** One frame's outcome. */
export interface TerrainFrame {
  /** The instanced draw, or `null` before the material is ready or with nothing to draw. */
  readonly draw: DrawItem | null;
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
  /** Its instanced draw, or `null`: before the material is ready, with nothing it sees, or faulted. */
  readonly draw: DrawItem | null;
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
  /** The drawn patches' keys the holds are made from, reused from selection to selection. */
  readonly drawnKeys: PatchKey[];
  readonly pool: TerrainPool;
  readonly offBaked: () => void;
  readonly offFailed: () => void;
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
  /** Each level's morph band, start then end, at the view's tolerance ({@link effectiveTauPx}). */
  readonly morph: Float64Array;
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
  /** The draw set's index of each of this frame's instance records, in their order (R10.T9.b). */
  readonly drawn: number[];
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

/** No caster passes: a frame that draws no cascades. */
const NO_PASSES: ReadonlyArray<DepthLayerPass> = [];

/** The cascades' indices, nearest first. */
const CASCADES: readonly [CascadeIndex, CascadeIndex, CascadeIndex, CascadeIndex] = [0, 1, 2, 3];

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
  readonly drawSet: DrawSet;
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
  readonly #onSelect: ((input: SelectionInput, selection: Selection) => void) | null;
  /** The observers ({@link TerrainPass.observe}), replaced whole on each change. */
  #observers = NO_OBSERVERS;
  readonly #frustumMargin: TerrainFrustumMargin;
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
  /** Each view's own frame state, by its name. */
  readonly #views = new Map<TerrainViewId, ViewState>();
  /** Each view's camera at the last step, for its motion ({@link frustumMarginRad}). */
  readonly #stepped = new Map<TerrainViewId, SteppedCamera>();
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

  constructor(options: TerrainPassOptions) {
    this.#engine = options.engine;
    this.#setting = options.setting;
    this.#terrain = options.terrain ?? TERRAIN_SETTINGS[options.setting];
    this.#planet = options.planet;
    this.#source = options.source;
    this.#shading = terrainShadingOf(options.source);
    this.#materials = this.#shading === "spike" ? null : (options.materials ?? null);
    this.#createPool = options.createPool;
    this.#survey = options.survey ?? null;
    this.#measureSelection = options.measureSelection ?? false;
    this.#onResident = options.onResident ?? null;
    this.#onSelect = options.onSelect ?? null;
    this.#frustumMargin = options.frustumMargin ?? "none";
    this.#reliefM = options.reliefM ?? ((): number | null => null);
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
  get lastStep(): Pick<TerrainStep, "selection" | "drawSet"> | null {
    const selected = this.#selected;
    return selected === null || this.#faulted
      ? null
      : { selection: selected.selection, drawSet: selected.drawSet };
  }

  /**
   * Listens to the pass's selections and to the patches it makes resident, from the next of each
   * on, called where its options' `onSelect` and `onResident` are (plan R10, R10.T14.c: the descent
   * spike's metrics over a terrain session's pass, which the session makes without them).
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
   * R10.T16.c): a view's draw with this material and its `occluderSlopePx` writes the drawn ground's
   * depth into the symbology's pass before its lines; `null` before it is compiled, and for R05's
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
    kind: TerrainShading | "depth" | "shadow",
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

  /** Whether `device`'s materials are wanted no more: a restore replaced it, or the pass went. */
  #superseded(device: Device): boolean {
    return device !== this.#device || this.#disposed;
  }

  /**
   * The body's level table as its field's latest block gives it (`FieldChange.levels`, the pool's
   * `fieldLevels`; R10.T16.b): selection reads its bounds from the next step on. A table equal to
   * the one held, or none, changes nothing.
   */
  setLevels(levels: Float64Array | null): void {
    if (levels === null || this.#disposed || sameLevels(this.#planet.levels, levels)) {
      return;
    }
    this.#planet = planetGeometry(this.#planet.figure, levels);
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
    let reselected = false;
    if (this.#due(views, input)) {
      this.#select(this.#withMargins(views), input);
      reselected = true;
    }
    for (const view of views) {
      this.#stepped.set(view.id, view.camera);
    }
    const selected = this.#selected;
    if (selected === null) {
      throw new Error("selection did not run");
    }
    const { selection, drawSet } = selected;
    if (reselected || this.#demandDirty) {
      const demand = selection.demand.filter((r) => !device.cache.has(patchKeyString(r.key)));
      // The rebakes ask again for resident patches, which the selection's demand leaves out.
      const asked = [...demand, ...device.rebakes.values()];
      device.asked.clear();
      for (const request of asked) {
        device.asked.set(patchKeyString(request.key), request);
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
   * above 0 and its draw is made it renders their four caster passes into the one set before it
   * returns: render this view's draw, in the bodies pass that follows, before another view's
   * `drawFor` draws its own cascades into the set.
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
      }
      return {
        draw: null,
        conditions: CLEAR,
        annunciation: null,
        fault: this.#faulted ? this.#cause : null,
        missing: false,
        standingIn: false,
        maskReady: false,
        cascades: null,
      };
    }
    const { selection, drawSet } = selected;
    const view = selected.views[bit];
    const mask = 1 << bit;
    const missing = (drawSet.missingViews & mask) !== 0;
    const standingIn = (drawSet.standingInViews & mask) !== 0;
    const set = this.#cascadesFor(state, input);
    const { draw, count, refused, passes } = this.#writeOrNone(
      state,
      mask,
      selected.nearContacts,
      input,
      set,
    );
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
    return {
      draw,
      conditions,
      annunciation,
      fault: null,
      missing,
      standingIn,
      // A draw a loss refused at its making withholds the mask, whatever it would have drawn.
      maskReady:
        !missing && !refused && (draw !== null || count === 0) && !this.#seesCodeZeroOpen(mask),
      cascades: set === null ? null : { set, passes },
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
    let set: CascadeSet;
    try {
      const under = heldGroundUnder(planet.figure, this.#heights)(camera);
      set = fitCascades({
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
      });
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
    const { draw, conditions, annunciation } = this.drawFor(DEFAULT_TERRAIN_VIEW, input);
    return { draw, drawSet, selection, reselected, conditions, annunciation };
  }

  /**
   * Releases view `id`'s own draw resources, as a view that no longer hands the body over; only a
   * step that leaves the view out does it, so that a view taken again always takes a selection,
   * and with it its morph bands (the batch review's C9).
   */
  #forgetView(id: TerrainViewId): void {
    this.#stepped.delete(id);
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
      this.#views.clear();
      this.#stepped.clear();
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
    if (device.airless !== null) {
      this.#engine.releaseTexture(device.airless.body);
    }
  }

  #makeDevice(resources?: TerrainResources): Device {
    const made =
      resources ??
      new TerrainResources(this.#engine, this.#terrain, this.#planet.figure, this.#shading);
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
      drawnKeys: [],
      pool,
      offBaked: pool.onBaked((bake) => {
        this.#store(device, bake);
      }),
      offFailed:
        pool.onFailed?.((failure) => {
          this.#givenUp(device, failure.keyString);
        }) ?? ((): void => undefined),
      airless:
        this.#shading === "lit" ? createAirlessTables(this.#engine, "terrain: no air") : null,
      material: null,
      depthMaterial: null,
      shadowMaterial: null,
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
    old.pool.terminate();
    // Nothing is resident now: every view's draw set misses its patches, so no mask is given.
    this.#settleAll(old);
    // The lost device's handles went with it: `ResilientEngine` drops their release.
    this.#releaseDevice(old);
    this.#heights.clear();
    this.#device = this.#makeDevice(old.resources);
    // Each view's records sized to the new layout's slots, its debounce kept; its cascades'
    // centres dropped, since the pass is remade (decision-r10-t9-split, ruling 7 amended).
    for (const [id, state] of this.#views) {
      this.#views.set(id, { ...this.#viewState(id), debounce: state.debounce });
    }
    this.#rangesVersion += 1;
    this.#selected = null;
    this.#demandDirty = true;
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
    const stored = device.cache.insert(bake);
    if (stored.kind !== "stored") {
      return;
    }
    if (stored.evicted !== null) {
      this.#forget(device, stored.evicted);
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
    });
    device.graticules.delete(keyString);
    device.rebakes.delete(keyString);
    this.#landed(device, keyString);
    this.#rangesVersion += 1;
    this.#demandDirty = true;
    this.#onResident?.(bake.key);
    const observers = this.#observers;
    for (let n = 0; n < observers.length; n += 1) {
      observers[n]?.onResident?.(bake.key);
    }
  }

  /** A patch no longer resident: evicted, or refused by its slot. */
  #forget(device: Device, keyString: string): void {
    device.residents.delete(keyString);
    device.givenUp.delete(keyString);
    device.graticules.delete(keyString);
    this.#heights.forget(keyString);
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
      uniforms: {
        bodyRotation,
        sunDirection: new Float32Array(4),
        sunRadiance: new Float32Array(4),
        atlas,
      },
      litUniforms: { bodyRotation, cameraBodyFixedM, atlas, ...cascadeUniforms() },
      casterUniforms: { bodyRotation, cameraBodyFixedM, atlas },
      centres: null,
      drawn: [],
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
      draw: null,
      drawDevice: null,
      drawnTable: null,
      drawnAtmosphere: null,
    };
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
   * camera at the last step and its nearest selected ground; unchanged under `none`.
   */
  #withMargins(views: ReadonlyArray<PassSelectionView>): ReadonlyArray<PassSelectionView> {
    if (this.#frustumMargin === "none") {
      return views;
    }
    const last = this.#selected;
    return views.map((view) => {
      const n = last?.views.findIndex((was) => was.id === view.id) ?? -1;
      const was = n < 0 ? undefined : last?.views[n];
      const nearestM =
        was !== undefined && Number.isFinite(was.nearestM)
          ? was.nearestM
          : (last?.selection.nearestSurveyByViewM[n] ?? Number.POSITIVE_INFINITY);
      return {
        ...view,
        frustumMarginRad: frustumMarginRad(view, this.#stepped.get(view.id) ?? null, nearestM),
      };
    });
  }

  #select(
    views: ReadonlyArray<PassSelectionView>,
    input: Pick<TerrainStepInput, "grounded">,
  ): void {
    const planet = this.#planet;
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
    };
    const startMs = this.#measureSelection ? performance.now() : 0;
    const selection = selectPatches(selectionInput);
    if (this.#measureSelection) {
      performance.measure(SELECT_MEASURE, { start: startMs, end: performance.now() });
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
      // At the view's τ, not the τ ÷ (1 + m) selection uses, raised where the budget binds.
      const state = this.#views.get(view.id);
      if (state !== undefined) {
        const morphView = {
          ...view,
          tauPx: effectiveTauPx(view.bandTauPx, selection.limitExcess, view.weight),
        };
        for (let level = 0; level <= MAX_LEVEL; level += 1) {
          const [start, end] = morphRangeM(planet, level, morphView);
          state.morph[2 * level] = start;
          state.morph[2 * level + 1] = end;
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
    const device = this.#device;
    // The same object every selection, rewritten in place; unseen forced patches are pinned by
    // `retain` but not drawn.
    const drawSet = device.resolver.resolve(selection);
    device.cache.retain(selection, drawSet);
    // The holds at the drawn set's coarse–fine edges, stand-ins included, the same for every view:
    // each view's bands are its own, and need not leave the coarse side at k = 0 there, nor the
    // fine side at 1 (decision-r10-t16-leans §9).
    const keys = device.drawnKeys;
    keys.length = 0;
    for (const { patch } of drawSet.patches) {
      keys.push(patch.key);
    }
    device.holds.update(keys);
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
      drawSet,
      nearContacts,
      views: selectedViews,
      grounded,
      rangesVersion: this.#rangesVersion,
      surveyVersion: this.#survey?.version ?? 0,
    };
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
    readonly draw: DrawItem | null;
    readonly count: number;
    readonly refused: boolean;
    readonly passes: ReadonlyArray<DepthLayerPass>;
  } {
    try {
      return {
        ...this.#write(this.#device, state, mask, contacts, input, cascades),
        refused: false,
      };
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return { draw: null, count: 0, refused: true, passes: NO_PASSES };
      }
      throw error;
    }
  }

  /**
   * Writes view `state`'s records, the drawn patches in `mask`, relative to its camera, and returns
   * its draw, or `null`, with how many patches it draws; and, where its lit draw receives
   * `cascades` (R10.T9.b), draws their caster passes, which it returns.
   */
  #write(
    device: Device,
    state: ViewState,
    mask: number,
    contacts: ReadonlyArray<GroundContact>,
    input: TerrainViewFrameInput,
    cascades: CascadeSet | null,
  ): {
    readonly draw: DrawItem | null;
    readonly count: number;
    readonly passes: ReadonlyArray<DepthLayerPass>;
  } {
    const { resources } = device;
    const { instances, morph, phases, drawn: drawnIndices } = state;
    const { cameraM } = input.view;
    const [r0, r1, r2] = input.view.rotation.rows;
    const drawSet = this.#selected?.drawSet;
    const wire = input.wire;
    const { holds } = device.holds;
    instances.clear();
    phases?.clear();
    drawnIndices.length = 0;
    if (drawSet !== undefined) {
      // `patches` holds exactly the `count` drawn patches (`DrawSetResolver`), and `holds` their
      // morph holds in the same order (`#select`).
      for (let n = 0; n < drawSet.count; n += 1) {
        const drawn = drawSet.patches[n];
        if (drawn === undefined || ((drawSet.views[n] ?? 0) & mask) === 0) {
          continue;
        }
        // R · origin − camera, component by component, so that a frame makes no vector a patch.
        const { patch } = drawn;
        const o = patch.originM;
        const level = patch.key.level;
        instances.pushXyz(
          patch.slot,
          r0.x * o.x + r0.y * o.y + r0.z * o.z - cameraM.x,
          r1.x * o.x + r1.y * o.y + r1.z * o.z - cameraM.y,
          r2.x * o.x + r2.y * o.y + r2.z * o.z - cameraM.z,
          morph[2 * level] ?? 0,
          morph[2 * level + 1] ?? 0,
          this.#parentSlotOf(device, patch.key),
          parentQuadrant(patch.key),
          holds[n] ?? 0,
        );
        drawnIndices.push(n);
        if (phases !== null && wire !== undefined) {
          this.#pushPhase(device, phases, drawn.keyString, patch.originM, wire);
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
    if (phases !== null) {
      resources.writePhases(phases, state.id);
    }
    const count = instances.count;
    if (device.material === null || count === 0) {
      return { draw: null, count, passes: NO_PASSES };
    }
    rotationMat4(input.view.rotation, state.uniforms.bodyRotation);
    let draw: DrawItem | null;
    let live: CascadeSet | null = null;
    switch (this.#shading) {
      case "lit": {
        const lit = this.#writeLit(device, state, input, cascades);
        draw = lit.draw;
        live = lit.live;
        break;
      }
      case "wire":
        // A wireframe view's frame with no contours to draw draws nothing (R10.T11).
        draw = wire === undefined ? null : this.#writeWire(device, state, device.material, wire);
        break;
      case "spike":
        draw = this.#writeSpike(device, state, device.material, input);
        break;
    }
    const passes =
      draw !== null && live !== null ? this.#drawCasters(device, state, input, live) : NO_PASSES;
    return { draw, count, passes };
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
   * A lit frame's uniforms and lights, its shadow cascades' fields (R10.T9.b), and its draw, made
   * again where its tables or its bound shadow texture changed; with the cascades its draw
   * receives, `null` where it receives none.
   *
   * @remarks
   * The draw receives `cascades` where their weight is above 0 and the caster material is made, the
   * set then made at its first need; it binds the set's layers while the set lives, and the
   * placeholder before. Otherwise its cascades' fields take w = 0, so that it samples nothing.
   */
  #writeLit(
    device: Device,
    state: ViewState,
    input: TerrainViewFrameInput,
    cascades: CascadeSet | null,
  ): { readonly draw: DrawItem | null; readonly live: CascadeSet | null } {
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
    const material = device.material;
    const drawable = table !== null && atmosphere !== null && material !== null;
    const live =
      drawable &&
      cascades !== null &&
      cascades.weight > 0 &&
      device.shadowMaterial !== null &&
      shadows.cascadeTarget() !== null
        ? cascades
        : null;
    packCascadeUniforms(live, viewAxisOf(input.view.orientation), state.litUniforms);
    if (table === null || atmosphere === null || material === null) {
      return { draw: null, live: null };
    }
    const shadow = shadows.bound();
    if (
      state.draw === null ||
      state.drawDevice !== device ||
      state.drawnTable !== table ||
      state.drawnAtmosphere !== atmosphere ||
      state.drawnShadow !== shadow
    ) {
      state.draw = this.#drawItem(device, state, material, { table, atmosphere, shadow });
      state.drawDevice = device;
      state.drawnTable = table;
      state.drawnAtmosphere = atmosphere;
      state.drawnShadow = shadow;
    }
    return { draw: state.draw, live };
  }

  /**
   * Draws view `state`'s casters into each of `set`'s cascades (plan R10, R10.T9.b;
   * decision-r10-t9b-design D4), one pass a cascade, and returns the passes.
   *
   * @remarks
   * Each cascade's casters are the view's own records this frame, in its order, whose patch's
   * bounding sphere (the resident bake's origin and bounding radius) meets the cascade's
   * light-space box with the caster reach ({@link castsInto}), each copied as it stands, so that a
   * caster is drawn with the view's own morph bands, holds and parents and stores the surface the
   * view draws. They are written into the view's caster buffer for the cascade and drawn with a
   * direct instance count, through the caster material, the lit mesh and the view's storage and
   * `Draw` values. Each pass, into the cascade's layer with its light rotation and projection,
   * draws the terrain's draw, where it has casters, and then the other casters'
   * ({@link TerrainShadows.casters}); every pass is drawn, clearing its layer, so that no other
   * view's casts remain in it. The set renders them under the view's name, each a submission of
   * its own, before this returns.
   */
  #drawCasters(
    device: Device,
    state: ViewState,
    input: TerrainViewFrameInput,
    set: CascadeSet,
  ): ReadonlyArray<DepthLayerPass> {
    const shadows = this.#shadows;
    const target = shadows?.target ?? null;
    const material = device.shadowMaterial;
    const drawSet = this.#selected?.drawSet;
    if (shadows === null || target === null || material === null || drawSet === undefined) {
      return NO_PASSES;
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
    // Each patch's centre less the camera, body-fixed, in `f64`, as the fit placed the cascades.
    const camera = rotateToBodyFixed(input.view.rotation, input.view.cameraM);
    const drawn = state.drawn;
    for (let record = 0; record < drawn.length; record += 1) {
      const drawnIndex = drawn[record];
      // A record whose patch the draw set no longer holds casts nothing.
      const patch = drawnIndex === undefined ? undefined : drawSet.patches[drawnIndex]?.patch;
      if (patch === undefined) {
        continue;
      }
      const o = patch.originM;
      const fromCamera = { x: o.x - camera.x, y: o.y - camera.y, z: o.z - camera.z };
      for (const index of CASCADES) {
        if (castsInto(set, index, fromCamera, patch.boundingRadiusM)) {
          casters.records[index].pushCopy(state.instances, record);
        }
      }
    }
    if (casters.items === null || casters.device !== device || casters.material !== material) {
      casters.items = this.#casterItems(device, state, material);
      casters.device = device;
      casters.material = material;
    }
    const items = casters.items;
    const passes = CASCADES.map((index): DepthLayerPass => {
      const records = casters.records[index];
      resources.writeCasters(state.id, index, records);
      const cascade = set.cascades[index];
      const others = shadows.casters.draws(set, index, state.id);
      const count = records.count;
      return {
        label: TERRAIN_SHADOW_PASS_LABEL,
        layer: index,
        viewRotation: cascade.viewRotation,
        projection: cascade.projection,
        draws: count > 0 ? [{ ...items[index], instanceCount: count }, ...others] : others,
      };
    });
    target.render(state.id, passes);
    return passes;
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

const ZERO_OFFSET = new Float32Array(3);

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
