/**
 * The terrain session (plan R10, R10.T16.a; decision-r10-live-path, D1): for one universe, one
 * engine and one request client, the coarse fields every view of `VIEW` wants, held by one pool of
 * height workers, and for each wanted body its survey's GPU coverage mask, its class map and its
 * detail seed, with the reliefs by which a body hands over and the surveys the label block reads.
 *
 * @remarks
 * - **What it holds.** T7's {@link CoarseFieldStore} over one {@link HeightWorkerPool} of Design note
 *   11's default size ({@link defaultHeightWorkers}), one a session whatever the views and bodies;
 *   for each wanted body a {@link CoverageMask} (T10.c), fed from the pool's `onFieldChange` (each
 *   block's newly surveyed cells, at once) and from the store's coverage, and forgotten when the
 *   workers drop the body's field; T10.d's {@link ClassMapStore}, which asks for a body's map once
 *   its detail seed is held; and the detail seed itself.
 * - **The views.** Each view's loop tells the session what it drew each frame it draws
 *   ({@link TerrainSession.noteView}: its scene, pose, field of view, presented size, style, the
 *   setting, its budget's streaming priority and its last plan's regimes), and the primary's frame
 *   steps it once ({@link TerrainSession.step}): T7's `follow` over every view's
 *   {@link FieldCandidate}s ({@link viewFieldCandidates}), each judged at its own style's τ, so that
 *   a wireframe view (4 px) and a photorealistic one (1 px) each have their own range.
 * - **The detail seed** of a server body is asked of its record, one `body_detail` a want; a kept
 *   scene's body carries its own (`ViewBody.detailSeed`). A body whose hooks are not modelled is
 *   wanted for its coverage alone: no class map. The atmosphere cache reads `body_detail` too:
 *   `VIEW` gives the session its requests through the `BodyDetailShare` it shares with the cache
 *   (plan R08, R08.T10.b), so that one request serves both ({@link TerrainSession}'s `#askSeed`).
 * - **The scene and the link.** {@link TerrainSession.noteSurfaceRevisions} takes a push's
 *   `surface_revisions`, {@link TerrainSession.resync} the scene topic opened again, and
 *   {@link TerrainSession.setLinkUp} the link's state: each asks the store for the new cells with the
 *   revision held.
 * - **What it gives the views**: {@link TerrainSession.classMaps} and {@link TerrainSession.reliefs},
 *   `PhotorealInputs`' `classMaps` and `reliefs`, for every view's frames.
 * - **Memory.** The masks and maps are under `coarse-field-gpu` (R12 Design note 6); the
 *   session's report ({@link TerrainSession.memory}) gives the workers' copies of the fields, the
 *   pool's kept blocks and the session's own GPU bytes by category.
 * - **The shadow cascades (R10.T9.b; Design note 11; decision-r10-t9b-design D2, D3 and D9).** The
 *   session holds the engine's one set of cascades, on the high setting alone, made at the first
 *   view-frame that draws them and let go at its teardown; the placeholder every lit draw binds
 *   where no set exists; and the registry of casters that are not terrain
 *   ({@link TerrainSession.shadowCasters}), all in one {@link TerrainShadows} that every lit pass
 *   draws with. Each view's frame draws its cascades for the one body that takes them
 *   (`LiveFrameTerrain.draws`), through its pass's `drawFor`, before its bodies pass.
 * - **The materials (R10.T14.d; decision-r10-t14a §1, decision-r10-t14d-stall).** The lit
 *   material, its depth material, on the high setting its caster into the shadow cascades
 *   (R10.T9.b), and the wireframe's, on the vertex paths of the setting it is
 *   made for, are compiled as it is made and after each restore ({@link TerrainMaterialSet}),
 *   before any hand-over, so that a body's hand-over compiles nothing and the terrain draws on its
 *   frame; every pass draws with those handles, and the session releases them at its teardown.
 * - **The passes (R10.T16.b; decision-r10-live-path, D3 and D4).** One {@link TerrainPass} a body,
 *   lit, shared by every view that hands the body over (T10.e's `terrain` regime in a
 *   photorealistic view's last plan), made at the body's first hand-over in any view, on its field
 *   header's figure and the workers' level table, baking through a lane of the session's one pool
 *   (`HeightWorkerPool.lane`), and disposed where the workers drop the body's field
 *   (`#fieldReleased`), not at a hand-back, and first at the teardown. Each step steps every pass
 *   once over the views that hand its body over, each at its own style's τ, presented size and
 *   weight (`ViewBudget.streamPriority`), with the scene's craft over it as its contacts
 *   ({@link sceneContacts}) on the ground its resident bakes hold; each view draws its own through
 *   {@link TerrainSession.drawTerrain}.
 * - **The survey and the mask (D6 and D7).** Each wanted body's {@link SurveyedCells}, which its
 *   pass's selection reads, take every block's cells and the store's coverage at once. Its GPU
 *   mask takes a block's cells once the rebakes the block named have landed where a pass draws
 *   the body (`TerrainPass.fieldChanged`), and the store's coverage once the workers have taken
 *   the blocks posted before it and no rebake is due; at once where no pass draws it.
 * - **The fault.** Once every height worker is lost, the passes draw nothing more, and every view
 *   that hands a body over shows `TERRAIN_NOT_AVAILABLE` on its label block's fault line
 *   ({@link TerrainSession.terrainFaults}, which {@link TerrainSession.subscribeFaults} follows), until
 *   `VIEW` is shown again and the session made anew. Likewise once a guarded computation, the
 *   session's step or a view's survey edges, has kept failing for `ANNUNCIATION_ONSET_MS`
 *   ({@link TerrainSession.failComputation}; R10.T16.e): every pass halted and none made again, and
 *   `TERRAIN_COMPUTATION_FAILED` on the same line. The first cause raised stands for the session.
 * - **Each view's frame (R10.T16.c).** Its `LiveFrameTerrain` (`frameTerrain.ts`) calls
 *   {@link TerrainSession.drawTerrain} with the view's `TerrainView` and lighting from its plan, and
 *   gives {@link TerrainSession.coverageMask}'s texture as its `surveyCoverage` only while the
 *   view's draw set of the body leaves nothing missing (`TerrainPass.maskReadyFor`); its draw
 *   list's survey edges read {@link TerrainSession.survey} and each pass's resident heights
 *   (`surveyEdgeViewsOf`).
 * - **The wireframe's passes (R10.T11; decision D10).** A wireframe view hands a body over where
 *   its relief subtends the wireframe's 4 px (`WIREFRAME_TERRAIN_TAU_PX`) at its limb, with R07's
 *   hysteresis ({@link wireframeHandovers}), judged here at each step from the view's report, since
 *   a wireframe frame plans no regimes. Each body so handed over has a pass of its own shading,
 *   baked without the class weights and the horizon map (`HeightSourceSetting.weights`
 *   `"skipped"`), through a lane of the same pool, shared by every wireframe view that hands it
 *   over and made and disposed as the lit one is; each wireframe view draws its own
 *   ({@link TerrainSession.drawWireTerrain}). The body's mask waits for both passes' rebakes.
 * - **The terrain readout (R10.T12; decision-r10-live-path, D10).** A view's `GROUND AT LINE OF
 *   SIGHT` is asked of the pool, one query of a worker over its field
 *   ({@link TerrainSession.groundReadout}), at the guide's 4 Hz. Its season, for `PAD FRICTION`'s
 *   `AT`, is the mean anomaly of the body's seasonal orbit at the instant asked: the body's own orbit
 *   from the `body_detail` record its detail seed is read from, or for a moon its planet's, asked
 *   once (decision-r10-t12-leans §6); none for a kept scene's body, whose record is not asked.
 */

import {
  type BodyIdHex,
  type DetailSeedHex,
  parseBodyId,
  type RequestClient,
  type SurfaceHeaderDto,
  type SurfaceRevisionDto,
  type UniverseIdHex,
  type UniverseTime,
} from "@hyperion/protocol";

import { add, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { type KeplerOrbit, meanAnomalyAt } from "../../lib/orbit";
import {
  bodyDetailSeed,
  bodyRecordOrbit,
  bodyRecordSeasons,
  type RecordSeasons,
  toBodyDetailRequest,
} from "../../lib/system/bodiesWire";
import { stepClassMapPoints } from "../bodies/classMapPoint";
import type { LitRegime } from "../bodies/regime";
import type { ViewBudget } from "../budget/viewBudget";
import type { CameraPose } from "../camera/pose";
import type { Viewport } from "../camera/projection";
import { type RenderStyle, sceneFrameFor, type ViewId } from "../camera/state";
import { differenceM, type ViewPosition } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import { rotateToBodyFixed } from "../coords/rotation";
import { type MemoryCategory, textureBytes, type TextureSpec } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { RenderEngine, TextureHandle } from "../engine/types";
import { isLitBody } from "../lighting/retarded";
import type { QualitySetting, TerrainSettings } from "../quality/qualitySetting";
import { cameraSceneOf, type ViewBody, type ViewCraft, type ViewScene } from "../scene/model";
import {
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
  type TerrainFault,
} from "./annunciation";
import { type ClassMapSource, ClassMapStore } from "./classMap";
import {
  CoarseFieldStore,
  type FieldCandidate,
  type FieldPool,
  type FieldStatus,
} from "./coarseFieldStore";
import { CoverageMask, type CoverageView } from "./coverage";
import { MAX_CONTACTS } from "./gpu/uniforms";
import {
  craftContact,
  finestPatchSizeM,
  type GroundContact,
  type GroundUnderFunction,
  heldGroundUnder,
} from "./grounded";
import {
  handoverRangeRatio,
  type HandoverReliefSource,
  handoverTauPx,
  handsOver,
  limbDistanceM,
  pixelAngleRad,
  storeReliefSource,
} from "./handover";
import { TerrainMaterialSet } from "./materialSet";
import { unreachable } from "./patchKey";
import { type BodyFigure, type BodyFixedVec3, planetGeometry } from "./planet";
import { PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "./priority";
import { MAX_SELECTION_VIEWS } from "./select";
import { type ShadowCasters, TerrainShadows } from "./shadows/terrainShadows";
import { SurveyedCells } from "./surveyLookup";
import {
  TerrainPass,
  type TerrainPassView,
  type TerrainViewFrame,
  type TerrainViewFrameInput,
} from "./terrainPass";
import { wireframeTerrainSettings } from "./terrainWire";
import { hullBoundingRadiusM } from "../wireframe/hulls";
import type { BakeSettings, SlopeBaselineM, SurfaceGroundReadout } from "./workers/messages";
import {
  defaultHeightWorkers,
  type FieldChange,
  type HeightWorkerLane,
  HeightWorkerPool,
  type WorkerLike,
} from "./workers/pool";
import type { WeightsLayerSetting } from "./workers/messages";

/**
 * What a session asks of its server: R09's `surface_field` for the fields and `body_detail` for
 * the detail seeds, through the link's request client or a kept scene's fixture (R10.T16.d).
 */
export type TerrainRequests = Pick<RequestClient, "request" | "requestBulk">;

/** What makes a session. */
export interface TerrainSessionOptions {
  /** The engine every view of the session draws with, on which its masks and maps are made. */
  readonly engine: RenderEngine;
  /** The universe the bodies are in; a session serves one. */
  readonly universe: UniverseIdHex;
  /** Its request client. */
  readonly requests: TerrainRequests;
  /**
   * The quality setting its views draw at, the launch's (R07.T17), on whose vertex paths the
   * materials of its passes are compiled as it is made (R10.T14.d; `materialSet.ts`).
   */
  readonly setting: QualitySetting;
  /** Whether the link is up as it starts; {@link TerrainSession.setLinkUp} follows it. */
  readonly linkUp: boolean;
  /**
   * Starts one height worker: the module worker `height.worker.ts` in the app, a fake in a test
   * (`HeightWorkerPoolOptions.createWorker`).
   */
  readonly createWorker: () => WorkerLike;
  /**
   * How many height workers its pool runs, at least one; Design note 11's default
   * ({@link defaultHeightWorkers} of `navigator.hardwareConcurrency`) where absent.
   */
  readonly workers?: number;
}

/**
 * What one view drew in one frame, as the session judges the fields it wants (Design note 16) and,
 * from R10.T16.b, the passes it draws.
 */
export interface TerrainViewReport {
  /** The scene it drew. */
  readonly scene: ViewScene;
  /** Its camera's pose in that scene (`runPose`). */
  readonly pose: CameraPose;
  /** Its horizontal field of view, rad, in (0, π). */
  readonly fovXRad: number;
  /**
   * The size it was presented at, device px: its canvas's (`presentedViewport`), at which a body's
   * hand-over range is reckoned for its fields (R05 Design note 23), whatever its internal scale.
   */
  readonly presentedPx: Viewport;
  /** The style its budget draws (`ViewBudget.style`), whose τ judges its range (Design note 13). */
  readonly style: RenderStyle;
  /**
   * The style its last frame was drawn in (`ViewFrameDrawn.drawnStyle`): the wireframe while its
   * photorealistic pipelines compile, or stand in after a device loss. Only a view whose last frame
   * was photorealistic hands a body over to the lit pass (the batch review's S8), and only one
   * whose last frame and budget are the wireframe's to the wireframe's (R10.T11); its fields are
   * judged by {@link TerrainViewReport.style}, so that a loss releases none.
   */
  readonly drawnStyle: RenderStyle;
  /** The quality setting it draws at, whose τ the photorealistic style takes. */
  readonly setting: QualitySetting;
  /** Its budget's terrain streaming priority (`ViewBudget.streamPriority`), for R10.T16.b's weights. */
  readonly streamPriority: ViewBudget["streamPriority"];
  /**
   * Its last photorealistic plan's regimes (`ViewFrameDrawer.regimes`), by which R10.T16.b's passes
   * take the bodies it hands over (T10.e's `terrain`).
   */
  readonly regimes: ReadonlyMap<BodyIdHex, LitRegime>;
  /**
   * The body whose terrain readout the view states, its `FRAME` body (plan R10, R10.T12; the
   * `PRIMARY` view's alone, decision-r10-t12-leans §3), or `null` or absent with none: wanted as the
   * view's local body is, so that its survey is asked wherever the camera is, and the view shows the
   * terrain's fault as a view that hands a body over does, unless the body's terrain is not
   * modelled ({@link TerrainSession.terrainFaults}; decision-r10-t12-commit3 §6).
   */
  readonly readoutBody?: BodyIdHex | null;
}

/**
 * A wanted body's survey, as the label block's survey line and terrain readout read it (R10.T12;
 * `readouts.ts`): where its field stands, its coverage and its header.
 *
 * @remarks
 * "A survey held" is a held header, from the field's first answer on, `SURVEY COVERAGE NONE`
 * included (decision-r10-t12-leans §7). A failure that leaves the field held keeps it, stale, and
 * the survey line states the failure after it (decision-r10-t16-leans §12(c)); one that drops the
 * field leaves no header.
 */
export interface TerrainSurvey {
  /**
   * The store's standing of the field: pending, not modelled or failed, its failure typed
   * (`FieldFailure`: the server's refusal or failure, or the console's check that failed).
   */
  readonly status: FieldStatus;
  /** Its coverage, or `null` before a field is held and after it is dropped. */
  readonly coverage: CoverageView | null;
  /** Its field header, or `null` before one is held and after the field is dropped. */
  readonly header: SurfaceHeaderDto | null;
  /**
   * Whether it may be behind the server's, so that its values read stale with their `S`: the link
   * is down, or has not been back long enough for an answer since (`CoverageView.stale`), or a
   * failure stands while the field is held, which nothing asks past until the body is wanted
   * afresh.
   */
  readonly stale: boolean;
}

/** A session's memory report (R10.T14; R12 Design note 6's line for the coarse field). */
export interface TerrainSessionMemory {
  /** The height workers' copies of every field together, bytes (`fieldResidentBytes`). */
  readonly workerFieldBytes: number;
  /**
   * The pool's own copy of every field's blocks on the render thread, bytes (`keptFieldBytes`;
   * R10.T7's "The pool's own copy").
   */
  readonly keptFieldBytes: number;
  /**
   * The GPU bytes of the textures the session holds, by category, the engine's own texel sizes
   * (`textureBytes`): its masks and maps under `coarse-field-gpu`, and on the high setting its
   * shadow cascades under `shadows` while the set lives and their placeholder under `other`
   * (R10.T9.b).
   */
  readonly gpuBytes: ReadonlyMap<MemoryCategory, number>;
}

/** The line of sight a view's terrain readout reads along, in the body's body-fixed frame (R10.T12). */
export interface GroundSight {
  /** Its origin, body-fixed metres: the camera's, or the hull's. */
  readonly originM: BodyFixedVec3;
  /** Its direction, body-fixed, of any length but zero. */
  readonly direction: BodyFixedVec3;
  /** How far along it the ground is looked for, metres: past the body's far side for `NO GROUND`. */
  readonly maxM: number;
}

/** A terrain readout's settings (R10.T12). */
export interface GroundReadoutOptions {
  /**
   * The baseline `SLOPE` is read over, metres (the guide's `BASELINE`): the one the `STYLE` panel's
   * `BASELINE` row sets for the `PRIMARY` view's readout, 2 as the display mounts (R10.T12.a).
   */
  readonly slopeBaselineM: SlopeBaselineM;
  /** The width of the pad the bearing capacity is taken under, metres: the reference pad's 1.0. */
  readonly padWidthM: number;
}

/**
 * One view's {@link FieldCandidate}s for T7's `follow`: each lit body of its scene (a planet, dwarf
 * planet or moon with a radius, `isLitBody`), whether it is the view's local body, and its range
 * ratio at the view's style's τ.
 *
 * @remarks
 * - `local` is the body frame the view's camera belongs in by R02's rule (`sceneFrameFor`), and the
 *   body whose terrain readout the view states ({@link TerrainViewReport.readoutBody}; R10.T12).
 * - `rangeRatio` is T10.e's `handoverRangeRatio`: the camera's distance to the body's limb
 *   (`limbDistanceM` of its distance from the centre and the body's equatorial radius) over the
 *   range where `reliefM` subtends τ pixels, τ the view's style's on its setting
 *   (`handoverTauPx`) and the pixel the presented size's centre pixel (`pixelAngleRad`). With no
 *   relief known (no header held) the ratio is infinite, so that a body with no header is wanted
 *   only as some view's local body (main's ruling for R10.T7).
 *
 * @param reliefM - A body's realised relief, m, from its held header, or `null` with none held.
 */
export function viewFieldCandidates(
  report: TerrainViewReport,
  reliefM: (body: BodyIdHex) => number | null,
): FieldCandidate[] {
  const { scene, pose } = report;
  const camera = cameraSceneOf(scene);
  const frame = sceneFrameFor(pose, camera);
  const local = frame.kind === "body" ? frame.body : null;
  const tauPx = handoverTauPx(report.style, report.setting);
  const radPerPx = pixelAngleRad(
    { orientation: pose.orientation, fovXRad: report.fovXRad },
    report.presentedPx,
  );
  return scene.bodies.filter(isLitBody).map((body): FieldCandidate => {
    const centreM = relativeToCamera(
      { kind: "body", body: body.id, m: vec3(0, 0, 0) },
      pose,
      camera.origins,
    );
    const limbM = limbDistanceM(norm(centreM), body.radiusM);
    return {
      body: body.id,
      local: body.id === local || body.id === report.readoutBody,
      rangeRatio: handoverRangeRatio(limbM, reliefM(body.id), tauPx, radPerPx),
    };
  });
}

/**
 * One view's part in a body's pass (R10.T16.b): its camera from the body's centre as it last drew,
 * in the body's non-rotating axes (the galactic axes, the body frame's), at its style's τ
 * (`handoverTauPx`), its presented size and its weight (`PRIMARY_VIEW_WEIGHT` or
 * `SECONDARY_VIEW_WEIGHT`, from its budget's `streamPriority`); `null` for a body whose rotation is
 * not known, which is never handed over.
 */
export function terrainPassViewOf(
  id: ViewId,
  report: TerrainViewReport,
  body: ViewBody,
): TerrainPassView | null {
  const rotation = body.rotation;
  if (rotation === null) {
    return null;
  }
  const centreM = relativeToCamera(
    { kind: "body", body: body.id, m: vec3(0, 0, 0) },
    report.pose,
    cameraSceneOf(report.scene).origins,
  );
  return {
    id,
    view: {
      rotation,
      cameraM: scale(centreM, -1),
      orientation: report.pose.orientation,
      fovXRad: report.fovXRad,
      viewport: report.presentedPx,
    },
    tauPx: handoverTauPx(report.style, report.setting),
    weight: report.streamPriority === "primary" ? PRIMARY_VIEW_WEIGHT : SECONDARY_VIEW_WEIGHT,
  };
}

/**
 * The bodies a wireframe view hands over to terrain (plan R10, R10.T11; Design note 13): each lit
 * body of its scene whose rotation is known and whose relief (`reliefM`) subtends the wireframe's
 * τ (`handoverTauPx`, 4 px on both settings) at its limb from the view's camera, at the presented
 * size's centre pixel, with R07's hysteresis (`handsOver`) from the bodies it handed over last
 * (`previous`). A view whose budget does not draw the wireframe, or whose last frame was not drawn
 * in it, hands nothing over in it: a photorealistic view standing in with the wireframe draws no
 * terrain, as before (the batch review's S8).
 *
 * @param reliefM - A body's relief by which it hands over (`HandoverReliefSource.reliefM`), or
 *   `null` where it never does.
 */
export function wireframeHandovers(
  report: TerrainViewReport,
  reliefM: (body: BodyIdHex) => number | null,
  previous: ReadonlySet<BodyIdHex>,
): Set<BodyIdHex> {
  const handed = new Set<BodyIdHex>();
  if (report.style !== "wireframe" || report.drawnStyle !== "wireframe") {
    return handed;
  }
  const { scene, pose } = report;
  const origins = cameraSceneOf(scene).origins;
  const tauPx = handoverTauPx("wireframe", report.setting);
  const radPerPx = pixelAngleRad(
    { orientation: pose.orientation, fovXRad: report.fovXRad },
    report.presentedPx,
  );
  for (const body of scene.bodies) {
    if (!isLitBody(body) || body.rotation === null) {
      continue;
    }
    const centreM = relativeToCamera(
      { kind: "body", body: body.id, m: vec3(0, 0, 0) },
      pose,
      origins,
    );
    const limbM = limbDistanceM(norm(centreM), body.radiusM);
    if (handsOver(limbM, reliefM(body.id), tauPx, radPerPx, previous.has(body.id))) {
      handed.add(body.id);
    }
  }
  return handed;
}

/**
 * Whether `scene` can place `position` against `body`, its centre (`differenceM` answers rather than
 * throws): every frame the two meet in is one the scene holds, its barycentre known where they meet
 * in the galactic frame (the batch review's S10).
 */
function placeableAgainst(position: ViewPosition, body: ViewBody, scene: ViewScene): boolean {
  const system = parseBodyId(body.id).system;
  let placeable: boolean;
  switch (position.kind) {
    case "body":
    case "body_fixed":
      placeable =
        scene.bodies.some((each) => each.id === position.body) &&
        (position.body === body.id || parseBodyId(position.body).system === system);
      break;
    case "system":
      placeable = position.system === system;
      break;
    case "galactic":
      placeable = scene.system === system && scene.barycentre !== null;
      break;
  }
  return placeable;
}

/**
 * A body's velocity at the epoch it is drawn at, m/s from the system's barycentre along the
 * galactic axes (decision-r10-t14c-leans §5): its present one where the scene holds it, as for the
 * ship's local body in a served scene (`ViewBody.presentVelocityMPerS`), and its retarded one
 * (`ViewBody.retarded`) otherwise; `null` where neither is known.
 */
function drawnVelocityOf(body: ViewBody): Vec3 | null {
  return body.presentVelocityMPerS ?? body.retarded?.velocityMPerS ?? null;
}

/**
 * A craft's velocity against `body` M, m/s along the galactic axes, chained through the system's
 * frame where the craft is held in another body's frame (plan R10, R10.T14.c; decision-r10-t16-leans
 * §13, Q3; decision-r10-t14c-leans §5): what {@link sceneContacts} judges its descent by. `null`
 * where it is not known or cannot be chained.
 *
 * @remarks
 * - **The rule.** Each body's velocity is taken at the epoch it is drawn at, V^drawn: the present
 *   one for the ship's local body, which a served scene draws at the present
 *   (`ViewBody.presentVelocityMPerS`, the `SceneFrame.localBodyVelocityMPerS` that the own ship's
 *   velocity in its frame is held against, decision-r10-ship-frame §2), and the retarded one for
 *   every other body, drawn where it is seen. In a kept scene no body carries a present velocity
 *   and every light time is 0, so the retarded velocity is the present one.
 *   - In M's own frames (its `body` or `body_fixed`), v, the craft's own.
 *   - In another body F's non-rotating frame, v + V_F^drawn − V_M^drawn: Galilean addition through
 *     the system's frame, so that a craft in the band of a small moon's Hill sphere, still held in
 *     its planet's frame, is judged against the moon. `null` where either body's is not known.
 *   - In another body's fixed frame, `null`: its velocity is against a frame turning at a rate the
 *     scene does not hold, so ω_F × r, 664.5 m/s at Phobos's distance from Mars, cannot be added
 *     back. Such a craft is a contact only while grounded.
 *   - In the system's frame, v − V_M^drawn; in the galactic frame, `null`.
 * - **Its error.** The contact's position is differenced through the drawn centres (in a served
 *   scene the local body's at the present, every other body's at its `apparentM`, which carries
 *   light time and aberration), and its velocity is of the same epochs: for the own ship held in
 *   its local body F's frame, v + V_F gives back its barycentric velocity, and the chain is exact
 *   against the picture to first order in v ÷ c. Against the physical present, a body M drawn at
 *   its emission errs by |a_M| τ_M to first order, a_M its acceleration in the system's frame and
 *   τ_M its light time to the scene's observer, the own ship in a served scene and 0 in a kept one.
 *   For a Phobos-like moon, GM_Mars ÷ a² = 0.4873 m/s² at a = 9,375 km:
 *   - the own ship in the band of its Hill sphere, 15.5 km from its centre and still held in the
 *     planet's frame: 0.4873 m/s² × 51.70 µs ≈ 2.5 × 10⁻⁵ m/s;
 *   - another craft at the moon, seen from the planet: 0.4873 m/s² × 31.27 ms ≈ 0.0152 m/s.
 *
 *   Nearer the moon than 0.9 of its Hill radius, 14.68 km at pericentre, the own ship is held in
 *   its frame: the moon is then the local body, drawn at the present, and τ_M is 0. F's retarded
 *   velocity in place of its present one would add |a_F| τ_F, 8.0 × 10⁻⁵ m/s at Phobos but 1.4 m/s
 *   for a moon 2.5 radii from a Jupiter-size planet 0.05 au from a Sun-like star.
 * - **Deferred: craft at the present against bodies at their emission.** A craft other than the
 *   own ship is placed at the scene's time, while every body but the local one is drawn where it is
 *   seen, so a contact against a moon seen across light time τ stands off by about |V_M − V_obs| τ,
 *   V_obs the observer's velocity: 66.8 m for a craft at a Phobos-like moon seen by an observer
 *   co-moving with its Mars-like planet, 2,137 m/s over 31.27 ms, beyond the held radius. No scene
 *   reaches it today: the server's craft source is `NoCraft`, and a kept scene has no light time
 *   (decision-r10-t14c-leans, deferred finding 1).
 * - This resolves decision-r10-ship-frame's finding 4 as it was worded: for the local body, which
 *   is drawn at the present, the present velocity is the reference.
 */
export function craftVelocityAgainst(
  craft: ViewCraft,
  body: ViewBody,
  scene: ViewScene,
): Vec3 | null {
  const v = craft.velocityMPerS;
  if (v === null) {
    return null;
  }
  const position = craft.pose.position;
  let against: Vec3 | null;
  switch (position.kind) {
    case "body":
      if (position.body === body.id) {
        against = v;
      } else {
        const frame = scene.bodies.find((each) => each.id === position.body);
        const frameVelocity = frame === undefined ? null : drawnVelocityOf(frame);
        const bodyVelocity = drawnVelocityOf(body);
        against =
          frameVelocity === null || bodyVelocity === null
            ? null
            : add(v, sub(frameVelocity, bodyVelocity));
      }
      break;
    case "body_fixed":
      against = position.body === body.id ? v : null;
      break;
    case "system": {
      const bodyVelocity = drawnVelocityOf(body);
      against = bodyVelocity === null ? null : sub(v, bodyVelocity);
      break;
    }
    case "galactic":
      against = null;
      break;
  }
  return against;
}

/**
 * The scene's craft over `body` as its pass's contacts (plan R10, R10.T16.b; R05 Design note 9):
 * each grounded or descending (`isDescending`) over the ground `ground` gives, at its hull's
 * bounding radius (`TEST HULL`'s until hulls are defined), passed at the ground beneath it
 * (`craftContact`), at most `MAX_CONTACTS` of them; none for a body whose rotation is not known. A
 * craft the scene cannot place against the body is passed over; one held in another body's frame
 * is judged against this one, its velocity chained through each body's at the epoch it is drawn at
 * ({@link craftVelocityAgainst}, R10.T14.c).
 *
 * @param ground - The ground under a body-fixed point: the workers' where the pass holds it
 *   (`heldGroundUnder` of its resident heights, R10.T14.c), the datum's elsewhere
 *   (decision-r10-t16-contracts §2).
 * @param patchSizeM - The finest patch's edge on the body, metres (`finestPatchSizeM`).
 */
export function sceneContacts(
  scene: ViewScene,
  body: ViewBody,
  ground: GroundUnderFunction,
  patchSizeM: number,
): GroundContact[] {
  const rotation = body.rotation;
  if (rotation === null) {
    return [];
  }
  const origins = cameraSceneOf(scene).origins;
  const contacts: GroundContact[] = [];
  for (const craft of scene.craft) {
    if (!placeableAgainst(craft.pose.position, body, scene)) {
      // A frame the scene does not hold: nothing to place the craft by.
      continue;
    }
    const fromCentreM = differenceM(
      craft.pose.position,
      { kind: "body", body: body.id, m: vec3(0, 0, 0) },
      origins,
    );
    const v = craftVelocityAgainst(craft, body, scene);
    const contact = craftContact(
      rotateToBodyFixed(rotation, fromCentreM),
      v === null ? null : rotateToBodyFixed(rotation, v),
      hullBoundingRadiusM(craft.hull),
      ground,
      patchSizeM,
    );
    if (contact !== null && contacts.length < MAX_CONTACTS) {
      contacts.push(contact);
    }
  }
  return contacts;
}

/** What the session's masks and maps make on its engine. */
type TextureEngine = Pick<
  RenderEngine,
  "createTexture" | "writeTexture" | "releaseTexture" | "onRestored"
>;

/**
 * The session's textures, made and released through its engine, each held with its bytes and
 * category for the memory report.
 */
class SessionTextures implements TextureEngine {
  readonly #engine: TextureEngine;
  readonly #held = new Map<
    TextureHandle,
    { readonly bytes: number; readonly category: MemoryCategory }
  >();

  constructor(engine: TextureEngine) {
    this.#engine = engine;
  }

  createTexture(spec: TextureSpec): TextureHandle {
    const texture = this.#engine.createTexture(spec);
    this.#held.set(texture, { bytes: textureBytes(spec), category: spec.category });
    return texture;
  }

  writeTexture(
    texture: TextureHandle,
    origin: GPUOrigin3D,
    size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.#engine.writeTexture(texture, origin, size, data);
  }

  releaseTexture(texture: TextureHandle): void {
    this.#held.delete(texture);
    this.#engine.releaseTexture(texture);
  }

  onRestored(listener: () => void): () => void {
    return this.#engine.onRestored(listener);
  }

  /** The bytes of the textures held, by category. */
  bytesByCategory(): ReadonlyMap<MemoryCategory, number> {
    const bytes = new Map<MemoryCategory, number>();
    for (const { bytes: each, category } of this.#held.values()) {
      bytes.set(category, (bytes.get(category) ?? 0) + each);
    }
    return bytes;
  }
}

/** Where a wanted body's detail seed stands. */
type SeedState =
  /** Held: its class map is wanted with it. */
  | { readonly kind: "held"; readonly seed: DetailSeedHex }
  /** Asked of `body_detail` at `time`, the answer awaited. */
  | { readonly kind: "asking"; readonly time: UniverseTime; readonly cancel: () => void }
  /** To be asked at `time` once the link returns. */
  | { readonly kind: "waiting"; readonly time: UniverseTime }
  /** None: its hooks are not modelled, or the record was refused; its coverage alone is wanted. */
  | { readonly kind: "none" };

/** Where the orbit that sets a wanted body's seasons stands (R10.T12's readout). */
type SeasonState =
  /** Held: the body's own orbit, or its planet's for a moon. */
  | { readonly kind: "held"; readonly orbit: KeplerOrbit }
  /** A moon's planet's record asked of `body_detail` at `time`, the answer awaited. */
  | {
      readonly kind: "asking";
      readonly planet: BodyIdHex;
      readonly time: UniverseTime;
      readonly cancel: () => void;
    }
  /** A moon's planet's record to be asked at `time` once the link returns. */
  | { readonly kind: "waiting"; readonly planet: BodyIdHex; readonly time: UniverseTime }
  /**
   * None known: the record not yet answered, a kept scene's body, whose record is not asked, or no
   * orbit the client can propagate on the record.
   */
  | { readonly kind: "none" };

/** A wanted body as the session holds it. */
interface SessionBody {
  /** Its survey's GPU coverage mask. */
  readonly mask: CoverageMask;
  /** Its survey as its pass's selection reads it, at once (R10.T16.b). */
  readonly survey: SurveyedCells;
  seed: SeedState;
  /** The orbit that sets its seasons, from the record its seed is read from (R10.T12). */
  season: SeasonState;
  /**
   * The store's latest coverage held back from the mask while its pass's rebakes are due, with the
   * blocks posted when it came, which the workers must take first.
   */
  pendingCoverage: { readonly coverage: CoverageView; readonly posted: number } | null;
  /** Rises when its field is dropped, so that a block's rebakes landing after reach no mask. */
  epoch: number;
}

/** A view that hands a body over, with the report and the body as it drew them. */
interface HandingView {
  readonly id: ViewId;
  readonly report: TerrainViewReport;
  readonly body: ViewBody;
}

/**
 * A body's passes by their shading (R10.T11): the photorealistic style's, lit, and the
 * wireframe's.
 */
type PassShading = "lit" | "wire";

/** No view shows the terrain's fault. */
const NO_FAULTS: ReadonlyMap<ViewId, TerrainFault> = new Map();

/** The views of `handing`, each once. */
function handingIds(
  ...handing: ReadonlyArray<ReadonlyMap<BodyIdHex, ReadonlyArray<HandingView>>>
): ViewId[] {
  return [
    ...new Set(
      handing.flatMap((each) => [...each.values()].flatMap((views) => views.map(({ id }) => id))),
    ),
  ];
}

/** No body handed over. */
const NO_BODIES: ReadonlySet<BodyIdHex> = new Set();

/** Whether a request ended because the client cancelled it (the protocol's own codes). */
function cancelled(code: string): boolean {
  return code === "aborted" || code === "superseded";
}

/**
 * One source's terrain for every view of `VIEW` (plan R10, R10.T16.a; see the module's remarks):
 * made by `useTerrainSession` for the server's scene of the open universe, or a kept scene's
 * terrain fixture, and disposed with the engine, the universe or a change of source.
 *
 * @remarks
 * Every method but the getters does nothing once the session is disposed, since a view's loop may
 * run a frame on a session its source has just let go.
 */
export class TerrainSession {
  readonly #universe: UniverseIdHex;
  readonly #requests: TerrainRequests;
  readonly #textures: SessionTextures;
  readonly #pool: HeightWorkerPool;
  readonly #store: CoarseFieldStore;
  readonly #classMaps: ClassMapStore;
  readonly #reliefs: HandoverReliefSource;
  readonly #engine: RenderEngine;
  /** The materials every pass draws with, compiled at its making on its setting (R10.T14.d). */
  readonly #materials: TerrainMaterialSet;
  /** The shadow cascades every lit pass draws with, on the high setting (R10.T9.b). */
  readonly #shadows: TerrainShadows;
  readonly #bodies = new Map<BodyIdHex, SessionBody>();
  readonly #reports = new Map<ViewId, TerrainViewReport>();
  /** Each handed-over body's pass (R10.T16.b), from its first hand-over to its field's release. */
  readonly #passes = new Map<BodyIdHex, TerrainPass>();
  /**
   * Each body handed over in the wireframe's pass (R10.T11), from its first hand-over in a
   * wireframe view to its field's release.
   */
  readonly #wirePasses = new Map<BodyIdHex, TerrainPass>();
  /** The bodies each wireframe view handed over at the last step, its hysteresis's (R10.T11). */
  readonly #wireHanded = new Map<ViewId, ReadonlySet<BodyIdHex>>();
  readonly #faultListeners = new Set<() => void>();
  readonly #unsubscribe: ReadonlyArray<() => void>;
  readonly #workerCount: number;
  #faults: ReadonlyMap<ViewId, TerrainFault> = NO_FAULTS;
  /** The terrain's fault by its first cause raised, which stands for the session; `null` before. */
  #cause: TerrainFault | null = null;
  /** A guarded computation kept failing (R10.T16.e): no pass is stepped or made again. */
  #computationFailed = false;
  #linkUp: boolean;
  #disposed = false;

  /** @throws Error for a worker count that is not a whole number of at least one. */
  constructor(options: TerrainSessionOptions) {
    this.#universe = options.universe;
    this.#requests = options.requests;
    this.#linkUp = options.linkUp;
    this.#engine = options.engine;
    this.#textures = new SessionTextures(options.engine);
    this.#workerCount =
      options.workers ?? defaultHeightWorkers(globalThis.navigator.hardwareConcurrency);
    const pool = new HeightWorkerPool({
      workers: this.#workerCount,
      createWorker: options.createWorker,
    });
    this.#pool = pool;
    // The store posts through the session, so that the workers' dropping a field reaches it.
    const fields: FieldPool = {
      fieldInit: (body, block) => {
        pool.fieldInit(body, block);
      },
      fieldChunk: (body, block) => {
        pool.fieldChunk(body, block);
      },
      fieldRelease: (body) => {
        this.#fieldReleased(body);
        pool.fieldRelease(body);
      },
      onFieldRefused: (listener) => pool.onFieldRefused(listener),
    };
    this.#store = new CoarseFieldStore({
      universe: options.universe,
      requests: options.requests,
      pool: fields,
      linkUp: options.linkUp,
    });
    this.#classMaps = new ClassMapStore(this.#textures, pool);
    this.#reliefs = storeReliefSource(this.#store);
    this.#unsubscribe = [
      pool.onFieldChange((change) => {
        this.#blockTaken(change);
      }),
      this.#store.onRevision((body, coverage) => {
        this.#coverageTaken(body, coverage);
      }),
      this.#textures.onRestored(() => {
        this.#restored();
      }),
    ];
    // Makes nothing until a frame needs it and cannot throw; before any pass, so that its restore
    // listener forgets the lost device's set before any pass's resources are made again.
    this.#shadows = new TerrainShadows(options.engine, { cascades: options.setting === "high" });
    // Last, so that nothing before it can throw and leave it compiling, and before any pass, so that
    // its restore listener runs before any pass's resources' (`materialSet.ts`).
    this.#materials = new TerrainMaterialSet(options.engine, options.setting);
  }

  /**
   * The registry of the shadow cascades' casters that are not terrain (plan R10, R10.T9.b;
   * decision-r10-t9b-design D9): craft and R11's rocks add a provider of each cascade's draws, which
   * every view's caster passes draw after the terrain's. Nothing in the app fills it yet.
   */
  get shadowCasters(): ShadowCasters {
    return this.#shadows.casters;
  }

  /**
   * The class maps every view's lit bodies' discs shade with (T10.d's `PhotorealInputs.classMaps`):
   * a wanted body's once its detail seed is held and its field answers one.
   */
  get classMaps(): ClassMapSource {
    return this.#classMaps;
  }

  /**
   * The reliefs by which every view's lit bodies hand over (T10.e's `PhotorealInputs.reliefs`,
   * `storeReliefSource` over the session's store).
   */
  get reliefs(): HandoverReliefSource {
    return this.#reliefs;
  }

  /** The bodies whose fields are wanted, in the order first wanted. */
  get wanted(): ReadonlySet<BodyIdHex> {
    return this.#store.wanted;
  }

  /** How many height workers its one pool runs (Design note 11's count by default). */
  get workerCount(): number {
    return this.#workerCount;
  }

  /**
   * Settles once the materials its passes draw with on its setting have compiled on the device it
   * was made on, or failed to, with how many compiled (R10.T14.d; `TerrainMaterialSet.compiled`):
   * the views check's terrain run starts its phases after it.
   */
  get compiled(): Promise<number> {
    return this.#materials.compiled;
  }

  /** When its materials' compile began, as it was made, `performance.now()` ms (R10.T14.d). */
  get compileStartedAtMs(): number {
    return this.#materials.startedAtMs;
  }

  /** The bodies with a terrain pass: each handed over since its field was last taken (R10.T16.b). */
  get passBodies(): ReadonlySet<BodyIdHex> {
    return new Set(this.#passes.keys());
  }

  /**
   * The bodies with a wireframe pass: each handed over in a wireframe view since its field was last
   * taken (R10.T11).
   */
  get wirePassBodies(): ReadonlySet<BodyIdHex> {
    return new Set(this.#wirePasses.keys());
  }

  /**
   * The views whose label blocks show the terrain's fault after any graphics fault
   * (decision-r10-t16-contracts §7; decision-r10-t16-leans §8), each with its text, the session's
   * first cause: every view that hands a body over, in either style, and every view whose report
   * names a body whose terrain readout it states (`TerrainViewReport.readoutBody`, R10.T12) unless
   * that body's terrain is not modelled (decision-r10-t12-commit3 §6), once
   * every height worker is lost (`TERRAIN_NOT_AVAILABLE`) or a guarded computation has kept failing
   * (`TERRAIN_COMPUTATION_FAILED`, {@link TerrainSession.failComputation}); the same object until
   * it changes, for `useSyncExternalStore`.
   */
  get terrainFaults(): ReadonlyMap<ViewId, TerrainFault> {
    return this.#faults;
  }

  /**
   * The session's terrain fault, its first cause, whichever views show it, or `null` while none is
   * raised (R10.T12): under it no view states a terrain readout (decision-r10-t12-leans §5).
   */
  get terrainFault(): TerrainFault | null {
    if (this.#cause !== null) {
      return this.#cause;
    }
    return this.#pool.lostWorkers === this.#pool.workerCount ? TERRAIN_NOT_AVAILABLE : null;
  }

  /**
   * Listens for a change of {@link TerrainSession.terrainFaults}; returns the unsubscribe. Stable in
   * identity, for `useSyncExternalStore`.
   */
  readonly subscribeFaults = (listener: () => void): (() => void) => {
    this.#faultListeners.add(listener);
    return () => {
      this.#faultListeners.delete(listener);
    };
  };

  /** `body`'s terrain pass, from its first hand-over to its field's release, or `null`. */
  pass(body: BodyIdHex): TerrainPass | null {
    return this.#passes.get(body) ?? null;
  }

  /**
   * View `view`'s draw of `body`'s terrain this frame (R10.T16.b; R10.T16.c's `FrameTerrain`
   * calls it): its pass's `drawFor` with the view's `TerrainView` and lighting from its plan, or
   * `null` where no pass draws the body, or once the session is disposed. Call it after the
   * frame's {@link TerrainSession.step}, once a view frame. On the high setting, for the body that
   * takes the view's cascades (`input.takesCascades`, R10.T9.b), it draws them into the session's
   * one set before it returns, so the view's bodies pass is drawn next.
   *
   * @throws RangeError if more contacts are given than the contact buffer holds.
   */
  drawTerrain(
    view: ViewId,
    body: BodyIdHex,
    input: TerrainViewFrameInput,
  ): TerrainViewFrame | null {
    if (this.#disposed) {
      return null;
    }
    return this.#passes.get(body)?.drawFor(view, input) ?? null;
  }

  /** `body`'s wireframe pass (R10.T11), from its first hand-over in a wireframe view, or `null`. */
  wirePass(body: BodyIdHex): TerrainPass | null {
    return this.#wirePasses.get(body) ?? null;
  }

  /**
   * Wireframe view `view`'s draw of `body`'s terrain this frame (R10.T11; the wireframe's frame
   * terrain calls it): its wireframe pass's `drawFor` with the view's `TerrainView` and its
   * contours' and graticule's input (`TerrainFrameInput.wire`), or `null` where no wireframe pass
   * draws the body, or once the session is disposed. Call it after the frame's
   * {@link TerrainSession.step}, once a view frame.
   *
   * @throws RangeError if more contacts are given than the contact buffer holds.
   */
  drawWireTerrain(
    view: ViewId,
    body: BodyIdHex,
    input: TerrainViewFrameInput,
  ): TerrainViewFrame | null {
    if (this.#disposed) {
      return null;
    }
    return this.#wirePasses.get(body)?.drawFor(view, input) ?? null;
  }

  /**
   * Takes what view `id` drew this frame; it judges the fields from the next
   * {@link TerrainSession.step} on, until the view draws again or is forgotten.
   */
  noteView(id: ViewId, report: TerrainViewReport): void {
    if (this.#disposed) {
      return;
    }
    this.#reports.set(id, report);
  }

  /** Forgets view `id`, closed or unmounted: its fields are judged without it from the next step. */
  forgetView(id: ViewId): void {
    this.#reports.delete(id);
    this.#wireHanded.delete(id);
  }

  /**
   * One step, once a primary frame before it draws: wants and releases the bodies' fields as every
   * view's last report judges them (T7's `follow` over {@link viewFieldCandidates}), makes or
   * releases each body's mask, detail seed and class map with its want, and steps each body's
   * pass (R10.T16.b) once over the views that hand its body over, making the pass at its first
   * hand-over. First it runs the frame's slice of the class-mapped points' work (R10.T14.f,
   * `stepClassMapPoints`): at most 2 ms of the elements' builds and the ratios' refreshes.
   *
   * @remarks
   * The views' debounces take their time from their own draws (`TerrainViewFrameInput.nowMs`), so
   * the step takes none.
   */
  step(): void {
    if (this.#disposed) {
      return;
    }
    // The frame's slice of the class-mapped points' builds and refreshes (R10.T14.f), which the
    // store queues as it takes each map, so that no frame's point makes them itself.
    stepClassMapPoints();
    // With no view's report there is nothing to judge the fields by: a loop that starts again on
    // a living session, before its first report, keeps every field (D1; the batch review's S2).
    if (this.#reports.size > 0) {
      const reliefM = (body: BodyIdHex): number | null =>
        this.#store.header(body)?.realised_relief_m ?? null;
      const candidates = [...this.#reports.values()].flatMap((report) =>
        viewFieldCandidates(report, reliefM),
      );
      this.#store.follow(candidates);
      this.#followWants();
    }
    if (this.#computationFailed) {
      // Halted for good (R10.T16.e): no pass is stepped or made, and the fault follows the views.
      this.#followFaults(handingIds(this.#handingViews(), this.#wireHandingViews()));
      return;
    }
    this.#stepPasses();
  }

  /**
   * Faults the session's terrain for a computation that keeps failing (plan R10, R10.T16.e;
   * decision-r10-t16-leans §8): the guard of the session's step, or of a view's survey edges, calls
   * it once their failures have lasted `ANNUNCIATION_ONSET_MS` (`TerrainFailureGuard`).
   *
   * @remarks
   * As lost workers do, and sticky until the session is made anew (`VIEW` shown again): every pass
   * is halted (`TerrainPass.halt`) and none is made again, so that the passes ask for and draw
   * nothing, each body is drawn by its figure and every mask is withheld; and every view that hands
   * a body over shows the fault at once, by the session's first cause
   * (`TERRAIN_COMPUTATION_FAILED`, or `TERRAIN_NOT_AVAILABLE` where every height worker was lost
   * first). Its steps follow the fields and the fault as before, and step no pass.
   */
  failComputation(): void {
    if (this.#disposed || this.#computationFailed) {
      return;
    }
    this.#computationFailed = true;
    this.#noteLostWorkers();
    this.#cause ??= TERRAIN_COMPUTATION_FAILED;
    const cause = this.#cause;
    for (const pass of [...this.#passes.values(), ...this.#wirePasses.values()]) {
      pass.halt(cause);
    }
    // Raised now, not at the next step, which may be what throws; where the views that hand a
    // body over cannot be told either, every view that has reported shows it.
    let views: ReadonlyArray<ViewId>;
    try {
      views = handingIds(this.#handingViews(), this.#wireHandingViews());
    } catch (error: unknown) {
      console.error("the views that hand a body over could not be told:", error);
      views = [...this.#reports.keys()];
    }
    this.#followFaults(views);
  }

  /**
   * Follows the link: down marks every survey stale, and up asks again for every field kept with
   * the revision held, and for every detail seed the link cut off.
   */
  setLinkUp(up: boolean): void {
    if (this.#disposed) {
      return;
    }
    this.#linkUp = up;
    this.#store.setLinkUp(up);
    if (!up) {
      return;
    }
    for (const [body, held] of this.#bodies) {
      if (held.seed.kind === "waiting") {
        this.#askSeed(body, held, held.seed.time);
      }
      if (held.season.kind === "waiting") {
        this.#askSeason(body, held, held.season.planet, held.season.time);
      }
    }
  }

  /** Takes a scene push's `surface_revisions`: a wanted body whose revision rose is asked the delta. */
  noteSurfaceRevisions(revisions: ReadonlyArray<SurfaceRevisionDto>): void {
    if (this.#disposed) {
      return;
    }
    this.#store.noteSurfaceRevisions(revisions);
  }

  /**
   * The scene topic was opened again after it could not go on (a resubscription after a sequence
   * fault, an unusable push or the server's ending it): every kept field is asked again with the
   * revision held (`CoarseFieldStore.resync`), as the link's return asks.
   */
  resync(): void {
    if (this.#disposed) {
      return;
    }
    this.#store.resync();
  }

  /**
   * A wanted body's survey for R10.T12's survey line and readout: the store's status, coverage and
   * header, stale while the link is down or a failure stands; `null` for a body not wanted.
   */
  survey(body: BodyIdHex): TerrainSurvey | null {
    const status = this.#store.status(body);
    if (status === null) {
      return null;
    }
    const coverage = this.#store.coverage(body);
    return {
      status,
      coverage,
      header: this.#store.header(body),
      stale: (coverage?.stale ?? !this.#linkUp) || status.failure !== null,
    };
  }

  /**
   * A wanted body's GPU coverage mask (T10.c, Design note 12): every cell surveyed by the blocks the
   * workers took and the store's answers, or `null` before any and for a body not wanted. R10.T16.c's
   * `FrameTerrain.surveyCoverage` gives it.
   */
  coverageMask(body: BodyIdHex): TextureHandle | null {
    return this.#bodies.get(body)?.mask.texture ?? null;
  }

  /** A wanted body's detail seed, once held; `null` before, for a body without hooks or not wanted. */
  detailSeed(body: BodyIdHex): DetailSeedHex | null {
    const seed = this.#bodies.get(body)?.seed;
    return seed?.kind === "held" ? seed.seed : null;
  }

  /**
   * The level table of `body`'s field as the workers hold it, settled (R10.T14.c): 25 × 4, as
   * `planetGeometry` takes it, the pool's `fieldLevels` once the workers have reported every block
   * posted so far of the field.
   *
   * @remarks
   * - It is `null` while a posted block is still unreported, so that a measurement made over it,
   *   the descent spike's floors (R10.T14.c), reads every block posted; and `null` once disposed,
   *   for a body whose field the pool does not keep (a body not wanted), and before any block's
   *   report.
   * - Its lowest and highest columns bound only the cells held, not the whole body: the held
   *   cells' elevations plus the most the level's and its parent's contributions can add,
   *   stretched to the datum, and exactly 0 and 0 before any cell
   *   (`crates/hyperion-surface/src/worker.rs`' `level_table`).
   *   Ground beyond the cells held is bounded by the body-wide ceiling instead (`reading.rs`'
   *   `ceiling_m`; decision-r10-t14c-leans §4).
   * - Its bound column is R09's `level_bound_m`, whatever is held.
   */
  levels(body: BodyIdHex): Float64Array | null {
    if (this.#disposed) {
      return null;
    }
    const blocks = this.#pool.fieldBlocks(body);
    return blocks !== null && blocks.reported === blocks.posted
      ? this.#pool.fieldLevels(body)
      : null;
  }

  /**
   * A lane of bakes of `bake`'s settings over the session's one pool (`HeightWorkerPool.lane`), its
   * queue, workers and fields, for a one-off measurement such as the descent spike's floors
   * (R10.T14.c); `null` once disposed. Its caller terminates it (`HeightWorkerLane.terminate`),
   * and the session's teardown, which terminates the pool, ends it too.
   */
  lane(bake: BakeSettings): HeightWorkerLane | null {
    return this.#disposed ? null : this.#pool.lane(bake);
  }

  /**
   * View `view`'s terrain readout of `body` where `sight` meets its ground (plan R10, T12;
   * decision-r10-live-path, D10): the slant range, the reading at its foot, the material there and
   * the ground's two properties, from one worker's query over the body's field
   * (`HeightWorkerPool.groundReadout`).
   *
   * @remarks
   * The season's mean anomaly is the body's seasonal orbit's at `time` (`meanAnomalyAt`), by which
   * the module takes the month of `PAD FRICTION`'s `AT`; none where no orbit is known, a kept
   * scene's body among them, so that a year of twelve months states no temperature then, and a
   * one-month year its one month.
   *
   * @param view - The view whose line of sight it is: only the primary asks in T12
   *   (decision-r10-t12-leans §3). A view the session holds no report of draws no terrain, so it
   *   reads none.
   * @param time - The instant the view lights the body at: its light's emission, the scene's time
   *   less the body's `retarded.lightTimeS` (`apparent.ts`' `emitted`), the scene's time in a kept
   *   scene (decision-r10-t12-leans §6 (c)).
   * @returns The readout; `null` once the session is disposed, for a view it holds no report of, a
   *   body not wanted, one whose detail seed is not held, or one whose field the pool does not keep
   *   (and where no worker runs). Every failure is a rejection, never a throw: with a `RangeError`
   *   for a malformed time (`fractionOfPeriod`), and with the module's message for a setting out of
   *   its range.
   */
  async groundReadout(
    view: ViewId,
    body: BodyIdHex,
    sight: GroundSight,
    time: UniverseTime,
    options: GroundReadoutOptions,
  ): Promise<SurfaceGroundReadout | null> {
    const held = this.#bodies.get(body);
    if (this.#disposed || !this.#reports.has(view) || held?.seed.kind !== "held") {
      return null;
    }
    return this.#pool.groundReadout(body, held.seed.seed, {
      originM: sight.originM,
      direction: sight.direction,
      maxM: sight.maxM,
      slopeBaselineM: options.slopeBaselineM,
      padWidthM: options.padWidthM,
      seasonMeanAnomalyRad:
        held.season.kind === "held" ? meanAnomalyAt(held.season.orbit, time) : undefined,
    });
  }

  /** The session's memory report: the fields' copies and its GPU bytes. */
  memory(): TerrainSessionMemory {
    const gpuBytes = new Map(this.#textures.bytesByCategory());
    for (const [category, bytes] of this.#shadows.bytesByCategory()) {
      gpuBytes.set(category, (gpuBytes.get(category) ?? 0) + bytes);
    }
    return {
      workerFieldBytes: this.#pool.fieldResidentBytes,
      keptFieldBytes: this.#pool.keptFieldBytes,
      gpuBytes,
    };
  }

  /**
   * Tears the session down, in order: its passes (their resources and meshes released, their lanes
   * ended), the terrain's materials (R10.T14.d), the shadow cascades and their placeholder
   * (R10.T9.b), the class-map store, the masks (with the detail
   * seeds' requests and the session's listeners), the store (its requests cancelled and its fields
   * released) and the pool (its workers terminated). Nothing is left: no worker, request, timer,
   * listener or GPU handle.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    for (const pass of [...this.#passes.values(), ...this.#wirePasses.values()]) {
      pass.dispose();
    }
    this.#passes.clear();
    this.#wirePasses.clear();
    this.#materials.dispose();
    this.#shadows.dispose();
    this.#wireHanded.clear();
    this.#faultListeners.clear();
    this.#classMaps.dispose();
    for (const unsubscribe of this.#unsubscribe) {
      unsubscribe();
    }
    for (const held of this.#bodies.values()) {
      if (held.seed.kind === "asking") {
        held.seed.cancel();
      }
      if (held.season.kind === "asking") {
        held.season.cancel();
      }
      held.mask.release();
    }
    this.#bodies.clear();
    this.#reports.clear();
    this.#store.dispose();
    this.#pool.terminate();
  }

  /**
   * A block the workers took: its newly surveyed cells reach the body's survey at once, which its
   * pass's selection reads, and its GPU mask, the masks' one feed from the workers, at once where no
   * pass draws the body, and otherwise once the rebakes the block names have landed (decision D7;
   * `TerrainPass.fieldChanged`), with the block's level table to the pass.
   */
  #blockTaken(change: FieldChange): void {
    const held = this.#bodies.get(change.body);
    if (held === undefined) {
      return;
    }
    held.survey.add(change);
    const passes = this.#passesOf(change.body);
    if (passes.length === 0) {
      held.mask.survey(change);
      return;
    }
    const epoch = held.epoch;
    // The mask takes the block once every pass that draws the body has its rebakes (R10.T11: the
    // lit one and the wireframe's alike).
    let unsettled = passes.length;
    for (const pass of passes) {
      pass.setLevels(change.levels);
      pass.fieldChanged(change, () => {
        unsettled -= 1;
        if (
          unsettled === 0 &&
          !this.#disposed &&
          this.#bodies.get(change.body) === held &&
          held.epoch === epoch
        ) {
          held.mask.survey(change);
          this.#flushCoverage(change.body, held);
        }
      });
    }
    this.#flushCoverage(change.body, held);
  }

  /** The passes that draw `body`: its lit pass and its wireframe pass, each where it has one. */
  #passesOf(body: BodyIdHex): TerrainPass[] {
    const passes: TerrainPass[] = [];
    for (const pass of [this.#passes.get(body), this.#wirePasses.get(body)]) {
      if (pass !== undefined) {
        passes.push(pass);
      }
    }
    return passes;
  }

  /**
   * The store's new coverage of a body: its cells reach the body's survey at once, and its GPU mask
   * at once where no pass draws the body, and otherwise once the workers have taken every block
   * posted before it and no rebake is due (decision D7), so that the mask never runs ahead of the
   * bakes drawn.
   */
  #coverageTaken(body: BodyIdHex, coverage: CoverageView): void {
    const held = this.#bodies.get(body);
    if (held === undefined) {
      return;
    }
    held.survey.merge(coverage);
    if (this.#passesOf(body).length === 0) {
      held.mask.update(coverage);
      return;
    }
    held.pendingCoverage = { coverage, posted: this.#pool.fieldBlocks(body)?.posted ?? 0 };
    this.#flushCoverage(body, held);
  }

  /**
   * Gives a body's mask the coverage held back, once every pass that draws it is settled and the
   * blocks taken.
   */
  #flushCoverage(body: BodyIdHex, held: SessionBody): void {
    const pending = held.pendingCoverage;
    if (pending === null || this.#passesOf(body).some((pass) => !pass.settled)) {
      return;
    }
    const blocks = this.#pool.fieldBlocks(body);
    if (blocks !== null && blocks.reported < pending.posted) {
      return;
    }
    held.pendingCoverage = null;
    held.mask.update(pending.coverage);
  }

  /**
   * The workers drop a body's field (released, refused, or held no longer for another history):
   * its pass is disposed (decision D4), and its survey and mask forget the cells, which no field
   * holds now.
   */
  #fieldReleased(body: BodyIdHex): void {
    const held = this.#bodies.get(body);
    if (held !== undefined) {
      held.epoch += 1;
      held.pendingCoverage = null;
      held.survey.clear();
    }
    this.#disposePasses(body);
    held?.mask.release();
  }

  /** Disposes of `body`'s passes, the lit one and the wireframe's, where it has them. */
  #disposePasses(body: BodyIdHex): void {
    for (const passes of [this.#passes, this.#wirePasses]) {
      const pass = passes.get(body);
      if (pass !== undefined) {
        passes.delete(body);
        pass.dispose();
      }
    }
  }

  /**
   * Steps each body's pass over the views that hand it over, making a pass at a body's first
   * hand-over, and follows the fault: every view that hands a body over shows it once every height
   * worker is lost.
   */
  #stepPasses(): void {
    const handing = this.#handingViews();
    const wireHanding = this.#wireHandingViews();
    for (const [shading, passes, views] of [
      ["lit", this.#passes, handing],
      ["wire", this.#wirePasses, wireHanding],
    ] as const) {
      for (const [body, each] of views) {
        const first = each[0];
        if (!passes.has(body) && first !== undefined) {
          this.#makePass(body, first, shading, passes);
        }
      }
      for (const [body, pass] of passes) {
        this.#stepPass(body, pass, views.get(body) ?? []);
      }
    }
    this.#followFaults(handingIds(handing, wireHanding));
  }

  /** Raises the lost workers' cause once every height worker is lost, unless a cause stands. */
  #noteLostWorkers(): void {
    if (this.#cause === null && this.#pool.lostWorkers === this.#pool.workerCount) {
      this.#cause = TERRAIN_NOT_AVAILABLE;
    }
  }

  /**
   * Follows the fault over `views`, the views that hand a body over, and the views whose label
   * blocks carry the survey of a body whose terrain is modelled, their reports' readout bodies
   * (R10.T12; decision-r10-t12-commit3 §6): each shows the session's cause once one is raised, and
   * none before; the listeners are told of a change.
   */
  #followFaults(views: ReadonlyArray<ViewId>): void {
    this.#noteLostWorkers();
    const cause = this.#cause;
    const showing = new Set(views);
    for (const [id, report] of this.#reports) {
      const body = report.readoutBody ?? null;
      // Not on the survey of a body whose terrain is not modelled (decision-r10-t12-commit3 §6).
      if (body !== null && (this.#store.status(body)?.notModelled ?? null) === null) {
        showing.add(id);
      }
    }
    const faults: ReadonlyMap<ViewId, TerrainFault> =
      cause === null || showing.size === 0
        ? NO_FAULTS
        : new Map([...showing].map((id) => [id, cause] as const));
    if (
      faults.size !== this.#faults.size ||
      [...faults].some(([id, text]) => this.#faults.get(id) !== text)
    ) {
      this.#faults = faults;
      for (const listener of this.#faultListeners) {
        listener();
      }
    }
  }

  /** Steps `body`'s pass over the views that hand it over, with the scene's craft as its contacts. */
  #stepPass(body: BodyIdHex, pass: TerrainPass, views: ReadonlyArray<HandingView>): void {
    const passViews = views
      .flatMap(({ id, report, body: seen }) => terrainPassViewOf(id, report, seen) ?? [])
      .slice(0, MAX_SELECTION_VIEWS);
    const seen = this.#sceneWith(body);
    const grounded =
      seen === null
        ? []
        : sceneContacts(
            seen.scene,
            seen.body,
            heldGroundUnder(pass.planet.figure, pass.heights),
            finestPatchSizeM(pass.planet),
          );
    pass.step({ views: passViews, grounded });
  }

  /**
   * The wireframe views that hand each wanted body over (R10.T11), in the order they first
   * reported: {@link wireframeHandovers} of each report, from the bodies it handed over at the last
   * step, which it keeps for the next.
   */
  #wireHandingViews(): Map<BodyIdHex, HandingView[]> {
    const handing = new Map<BodyIdHex, HandingView[]>();
    for (const [id, report] of this.#reports) {
      const handed = wireframeHandovers(
        report,
        (body) => (this.#bodies.has(body) ? this.#reliefs.reliefM(body) : null),
        this.#wireHanded.get(id) ?? NO_BODIES,
      );
      if (handed.size === 0) {
        this.#wireHanded.delete(id);
        continue;
      }
      this.#wireHanded.set(id, handed);
      for (const body of handed) {
        const seen = report.scene.bodies.find((each) => each.id === body);
        if (seen === undefined) {
          continue;
        }
        const views = handing.get(body) ?? [];
        views.push({ id, report, body: seen });
        handing.set(body, views);
      }
    }
    return handing;
  }

  /** A scene of the views' last reports holding `body`, with the body as it holds it; `null` where none does. */
  #sceneWith(body: BodyIdHex): { readonly scene: ViewScene; readonly body: ViewBody } | null {
    for (const { scene } of this.#reports.values()) {
      const found = scene.bodies.find((each) => each.id === body);
      if (found !== undefined) {
        return { scene, body: found };
      }
    }
    return null;
  }

  /**
   * The views that hand each wanted body over, in the order they first reported: T10.e's `terrain`
   * regime in a photorealistic view's last plan, for a body whose rotation is known (R10.T16.b;
   * the wireframe's are {@link TerrainSession}'s `#wireHandingViews`, R10.T11).
   */
  #handingViews(): Map<BodyIdHex, HandingView[]> {
    const handing = new Map<BodyIdHex, HandingView[]>();
    for (const [id, report] of this.#reports) {
      // The style the view drew, not the one it is to draw: a view standing in with the wireframe
      // after a device loss hands nothing over (S8).
      if (report.drawnStyle !== "photorealistic") {
        continue;
      }
      for (const [body, regime] of report.regimes) {
        if (regime !== "terrain" || !this.#bodies.has(body)) {
          continue;
        }
        const seen = report.scene.bodies.find((each) => each.id === body);
        if (seen === undefined || seen.rotation === null) {
          continue;
        }
        const views = handing.get(body) ?? [];
        views.push({ id, report, body: seen });
        handing.set(body, views);
      }
    }
    return handing;
  }

  /**
   * Makes `body`'s pass of `shading` into `passes` at its first hand-over in a view of its style,
   * on its field header's figure and the workers' level table, through a lane of the pool, once its
   * detail seed and header are held; nothing while the device is lost (the next step tries again).
   * The wireframe's bakes skip the class weights and the horizon map (decision D10).
   */
  #makePass(
    body: BodyIdHex,
    first: HandingView,
    shading: PassShading,
    passes: Map<BodyIdHex, TerrainPass>,
  ): void {
    const held = this.#bodies.get(body);
    const header = this.#store.header(body);
    const detailSeed = this.detailSeed(body);
    if (held === undefined || header === null || detailSeed === null) {
      return;
    }
    const figure: BodyFigure = {
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
      pole: null,
    };
    if (!(figure.polarRadiusM > 0 && figure.polarRadiusM <= figure.equatorialRadiusM)) {
      return;
    }
    let terrain: TerrainSettings | null;
    let weights: WeightsLayerSetting;
    let material: string;
    switch (shading) {
      case "lit":
        terrain = null;
        weights = "baked";
        material = "terrain material";
        break;
      case "wire":
        // The wireframe's cache, sized by its slots (`WIREFRAME_TERRAIN_SLOTS`, R10.T11).
        terrain = wireframeTerrainSettings(first.report.setting);
        weights = "skipped";
        material = "wireframe's terrain material";
        break;
    }
    let pass: TerrainPass;
    try {
      pass = new TerrainPass({
        engine: this.#engine,
        setting: first.report.setting,
        ...(terrain === null ? {} : { terrain }),
        planet: planetGeometry(figure, this.#pool.fieldLevels(body)),
        source: { kind: "body", body, detailSeed, weights },
        createPool: (bake) => this.#pool.lane(bake),
        survey: held.survey,
        // Compiled when the session was made, so that the hand-over compiles none (R10.T14.d).
        materials: this.#materials,
        // The engine's one set of cascades, which every view draws into in turn (R10.T9.b), and
        // the body's relief for their caster reach.
        ...(shading === "lit" ? { shadows: this.#shadows } : {}),
        reliefM: () => this.#store.header(body)?.realised_relief_m ?? null,
        // Each view's selection opened by its motion, so that no strip at its leading edge is
        // left unselected while its mask discards the reference surface there (S6).
        frustumMargin: "motion",
      });
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return;
      }
      throw error;
    }
    passes.set(body, pass);
    void pass.ready().catch((error: unknown) => {
      console.error(`the ${material} of ${body} failed:`, error);
    });
  }

  /** After a device loss: every mask made again from the cells it holds; nothing fetched or posted. */
  #restored(): void {
    for (const held of this.#bodies.values()) {
      held.mask.restore();
    }
  }

  /** Makes each newly wanted body's mask and detail seed, and releases each body no longer wanted. */
  #followWants(): void {
    const wanted = this.#store.wanted;
    for (const [body, held] of this.#bodies) {
      if (!wanted.has(body)) {
        this.#drop(body, held);
      }
    }
    for (const body of wanted) {
      if (!this.#bodies.has(body)) {
        this.#take(body);
      }
    }
  }

  /** Holds a newly wanted body: its mask, and its detail seed, its scene's own or asked for. */
  #take(body: BodyIdHex): void {
    const held: SessionBody = {
      mask: new CoverageMask(this.#textures, `coverage mask ${body}`),
      survey: new SurveyedCells(),
      seed: { kind: "none" },
      season: { kind: "none" },
      pendingCoverage: null,
      epoch: 0,
    };
    this.#bodies.set(body, held);
    const seen = this.#sceneBody(body);
    if (seen === null) {
      return;
    }
    const own = seen.body.detailSeed;
    if (own === undefined) {
      this.#askSeed(body, held, seen.time);
    } else {
      this.#seedHeld(body, held, own);
    }
  }

  /**
   * Releases a body no longer wanted: its pass (released with its field already, as a rule), its
   * detail seed's and its planet's record's requests, its class map and its mask.
   */
  #drop(body: BodyIdHex, held: SessionBody): void {
    this.#disposePasses(body);
    this.#bodies.delete(body);
    if (held.seed.kind === "asking") {
      held.seed.cancel();
    }
    if (held.season.kind === "asking") {
      held.season.cancel();
    }
    this.#classMaps.release(body);
    held.mask.release();
  }

  /** `body` as some view's scene holds it, with that scene's time; `null` where none does. */
  #sceneBody(body: BodyIdHex): { readonly body: ViewBody; readonly time: UniverseTime } | null {
    for (const { scene } of this.#reports.values()) {
      const found = scene.bodies.find((each) => each.id === body);
      if (found !== undefined) {
        return { body: found, time: scene.time };
      }
    }
    return null;
  }

  /** Holds `body`'s detail seed and wants its class map with it. */
  #seedHeld(body: BodyIdHex, held: SessionBody, seed: DetailSeedHex): void {
    held.seed = { kind: "held", seed };
    this.#classMaps.want(body, seed);
  }

  /**
   * Asks `body`'s record for its detail seed, one `body_detail` at the scene's time, or waits for
   * the link: the session's one fetch of the hooks, which `VIEW`'s `BodyDetailShare` joins with the
   * atmosphere cache's, answering it from the cache's reply where it holds one (plan R08,
   * R08.T10.b). The same record gives
   * the orbit that sets the body's seasons (R10.T12), or names a moon's planet, whose record is then
   * asked once ({@link TerrainSession}'s `#askSeason`).
   */
  #askSeed(body: BodyIdHex, held: SessionBody, time: UniverseTime): void {
    if (!this.#linkUp) {
      held.seed = { kind: "waiting", time };
      return;
    }
    const pending = this.#requests.request(toBodyDetailRequest(this.#universe, body, time));
    const asking: SeedState = {
      kind: "asking",
      time,
      cancel: () => {
        pending.cancel();
      },
    };
    held.seed = asking;
    const take = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (this.#bodies.get(body) !== held || held.seed !== asking) {
        return;
      }
      if (outcome.ok) {
        const { record } = outcome.response;
        const seed = bodyDetailSeed(record);
        if (seed === null) {
          held.seed = { kind: "none" };
        } else {
          this.#seedHeld(body, held, seed);
        }
        this.#takeSeasons(body, held, bodyRecordSeasons(record), time);
      } else if (outcome.error.code === "link_lost") {
        held.seed = { kind: "waiting", time };
      } else if (!cancelled(outcome.error.code)) {
        // Held until the body is wanted afresh, as the store holds a refused field.
        held.seed = { kind: "none" };
      }
    };
    void take().catch((error: unknown) => {
      console.error(`the detail seed of ${body} could not be taken:`, error);
    });
  }

  /**
   * Holds the orbit `body`'s record says sets its seasons: its own, or for a moon its planet's,
   * whose record is asked at `time`; none where the record holds none.
   */
  #takeSeasons(
    body: BodyIdHex,
    held: SessionBody,
    seasons: RecordSeasons,
    time: UniverseTime,
  ): void {
    switch (seasons.kind) {
      case "own":
        held.season = { kind: "held", orbit: seasons.orbit };
        return;
      case "planet":
        this.#askSeason(body, held, seasons.planet, time);
        return;
      case "none":
        held.season = { kind: "none" };
        return;
    }
    unreachable(seasons);
  }

  /**
   * Asks the record of `planet`, the planet of the moon `body`, for the orbit that sets the moon's
   * seasons (R10.T12; decision-r10-t12-leans §6 (b)), one `body_detail` at `time`, or waits for the
   * link; the season is none where the record is refused or holds no orbit the client propagates.
   */
  #askSeason(body: BodyIdHex, held: SessionBody, planet: BodyIdHex, time: UniverseTime): void {
    if (held.season.kind === "asking") {
      held.season.cancel();
    }
    if (!this.#linkUp) {
      held.season = { kind: "waiting", planet, time };
      return;
    }
    const pending = this.#requests.request(toBodyDetailRequest(this.#universe, planet, time));
    const asking: SeasonState = {
      kind: "asking",
      planet,
      time,
      cancel: () => {
        pending.cancel();
      },
    };
    held.season = asking;
    const take = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (this.#bodies.get(body) !== held || held.season !== asking) {
        return;
      }
      if (outcome.ok) {
        const orbit = bodyRecordOrbit(outcome.response.record);
        held.season = orbit === null ? { kind: "none" } : { kind: "held", orbit };
      } else if (outcome.error.code === "link_lost") {
        held.season = { kind: "waiting", planet, time };
      } else if (!cancelled(outcome.error.code)) {
        held.season = { kind: "none" };
      }
    };
    void take().catch((error: unknown) => {
      console.error(`the seasonal orbit of ${body}'s planet ${planet} could not be taken:`, error);
    });
  }
}
