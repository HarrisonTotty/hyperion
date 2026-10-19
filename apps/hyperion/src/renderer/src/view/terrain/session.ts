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
 *   wanted for its coverage alone: no class map. R08.T10.a reads `body_detail` too, and whichever
 *   of it and this lands second takes the other's fetch ({@link TerrainSession}'s `#askSeed`).
 * - **The scene and the link.** {@link TerrainSession.noteSurfaceRevisions} takes a push's
 *   `surface_revisions`, {@link TerrainSession.resync} the scene topic opened again, and
 *   {@link TerrainSession.setLinkUp} the link's state: each asks the store for the new cells with the
 *   revision held.
 * - **What it gives the views**: {@link TerrainSession.classMaps} and {@link TerrainSession.reliefs},
 *   `PhotorealInputs`' `classMaps` and `reliefs`, for every view's frames.
 * - **Memory.** The masks and maps are under `coarse-field-gpu` (R12 Design note 6); the
 *   session's report ({@link TerrainSession.memory}) gives the workers' copies of the fields, the
 *   pool's kept blocks and the session's own GPU bytes by category.
 * - **The passes (R10.T16.b; decision-r10-live-path, D3 and D4).** One {@link TerrainPass} a body,
 *   lit, shared by every view that hands the body over (T10.e's `terrain` regime in a
 *   photorealistic view's last plan), made at the body's first hand-over in any view, on its field
 *   header's figure and the workers' level table, baking through a lane of the session's one pool
 *   (`HeightWorkerPool.lane`), and disposed where the workers drop the body's field
 *   (`#fieldReleased`), not at a hand-back, and first at the teardown. Each step steps every pass
 *   once over the views that hand its body over, each at its own style's τ, presented size and
 *   weight (`ViewBudget.streamPriority`), with the scene's craft over it as its contacts
 *   ({@link sceneContacts}); each view draws its own through {@link TerrainSession.drawTerrain}.
 * - **The survey and the mask (D6 and D7).** Each wanted body's {@link SurveyedCells}, which its
 *   pass's selection reads, take every block's cells and the store's coverage at once. Its GPU
 *   mask takes a block's cells once the rebakes the block named have landed where a pass draws
 *   the body (`TerrainPass.fieldChanged`), and the store's coverage once the workers have taken
 *   the blocks posted before it and no rebake is due; at once where no pass draws it.
 * - **The fault.** Once every height worker is lost, the passes draw nothing more, and every view
 *   that hands a body over shows `TERRAIN_NOT_AVAILABLE` on its label block's fault line
 *   ({@link TerrainSession.terrainFaults}, which {@link TerrainSession.subscribeFaults} follows), until
 *   `VIEW` is shown again and the session made anew.
 * - **Each view's frame (R10.T16.c).** Its `LiveFrameTerrain` (`frameTerrain.ts`) calls
 *   {@link TerrainSession.drawTerrain} with the view's `TerrainView` and lighting from its plan, and
 *   gives {@link TerrainSession.coverageMask}'s texture as its `surveyCoverage` only while the
 *   view's draw set of the body leaves nothing missing (`TerrainPass.maskReadyFor`); its draw
 *   list's survey edges read {@link TerrainSession.survey} and each pass's resident heights
 *   (`surveyEdgeViewsOf`).
 */

import type {
  BodyIdHex,
  DetailSeedHex,
  RequestClient,
  SurfaceHeaderDto,
  SurfaceRevisionDto,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";

import { norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { bodyDetailSeed, toBodyDetailRequest } from "../../lib/system/bodiesWire";
import type { LitRegime } from "../bodies/regime";
import type { ViewBudget } from "../budget/viewBudget";
import type { CameraPose } from "../camera/pose";
import type { Viewport } from "../camera/projection";
import { type RenderStyle, sceneFrameFor, type ViewId } from "../camera/state";
import { differenceM } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import { rotateToBodyFixed } from "../coords/rotation";
import { type MemoryCategory, textureBytes, type TextureSpec } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { RenderEngine, TextureHandle } from "../engine/types";
import { isLitBody } from "../lighting/retarded";
import type { QualitySetting } from "../quality/qualitySetting";
import type { HullOutline } from "../scene/hull";
import { cameraSceneOf, type ViewBody, type ViewCraft, type ViewScene } from "../scene/model";
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
  datumGroundUnder,
  finestPatchSizeM,
  type GroundContact,
  type GroundUnderFunction,
} from "./grounded";
import {
  handoverRangeRatio,
  type HandoverReliefSource,
  handoverTauPx,
  limbDistanceM,
  pixelAngleRad,
  storeReliefSource,
} from "./handover";
import { type BodyFigure, planetGeometry } from "./planet";
import { PRIMARY_VIEW_WEIGHT, SECONDARY_VIEW_WEIGHT } from "./priority";
import { MAX_SELECTION_VIEWS } from "./select";
import { SurveyedCells } from "./surveyLookup";
import {
  TerrainPass,
  type TerrainPassView,
  type TerrainViewFrame,
  type TerrainViewFrameInput,
} from "./terrainPass";
import {
  defaultHeightWorkers,
  type FieldChange,
  HeightWorkerPool,
  type WorkerLike,
} from "./workers/pool";

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
  /** The quality setting it draws at, whose τ the photorealistic style takes. */
  readonly setting: QualitySetting;
  /** Its budget's terrain streaming priority (`ViewBudget.streamPriority`), for R10.T16.b's weights. */
  readonly streamPriority: ViewBudget["streamPriority"];
  /**
   * Its last photorealistic plan's regimes (`ViewFrameDrawer.regimes`), by which R10.T16.b's passes
   * take the bodies it hands over (T10.e's `terrain`).
   */
  readonly regimes: ReadonlyMap<BodyIdHex, LitRegime>;
}

/**
 * A wanted body's survey, as the label block's `SurveyLine` reads it (R10.T12): where its field
 * stands, its coverage and its header.
 */
export interface TerrainSurvey {
  /** The store's standing of the field: pending, not modelled or failed. */
  readonly status: FieldStatus;
  /** Its coverage, or `null` before a field is held and after it is dropped. */
  readonly coverage: CoverageView | null;
  /** Its field header, or `null` before one is held. */
  readonly header: SurfaceHeaderDto | null;
  /**
   * Whether it may be behind the server's: the link is down, or has not been back long enough for
   * an answer since (`CoverageView.stale`).
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
   * (`textureBytes`): its masks and maps under `coarse-field-gpu`.
   */
  readonly gpuBytes: ReadonlyMap<MemoryCategory, number>;
}

/**
 * One view's {@link FieldCandidate}s for T7's `follow`: each lit body of its scene (a planet, dwarf
 * planet or moon with a radius, `isLitBody`), whether it is the view's local body, and its range
 * ratio at the view's style's τ.
 *
 * @remarks
 * - `local` is the body frame the view's camera belongs in by R02's rule (`sceneFrameFor`).
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
      local: body.id === local,
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

/** A hull's bounding radius, m: its farthest vertex from its reference point (`TEST HULL`'s 11.06 m). */
export function hullBoundingRadiusM(hull: HullOutline): number {
  return Math.max(0, ...hull.vertices.map((v) => norm(v)));
}

/**
 * A craft's velocity against `body`, m/s along the galactic axes: its own where its frame is the
 * body's, less the body's (its retarded centre's) where its frame is the system's; `null` where
 * neither holds, or none is known.
 */
function velocityAgainst(craft: ViewCraft, body: ViewBody): Vec3 | null {
  const v = craft.velocityMPerS;
  if (v === null) {
    return null;
  }
  const position = craft.pose.position;
  let against: Vec3 | null;
  switch (position.kind) {
    case "body":
    case "body_fixed":
      against = position.body === body.id ? v : null;
      break;
    case "system":
      against = body.retarded === null ? null : sub(v, body.retarded.velocityMPerS);
      break;
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
 * craft the scene cannot place against the body is passed over.
 *
 * @param ground - The ground under a body-fixed point: the datum's (`datumGroundUnder`) until
 *   R10.T14.c gives it the workers' ground (decision-r10-t16-contracts §2).
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
    let fromCentreM: Vec3;
    try {
      fromCentreM = differenceM(
        craft.pose.position,
        { kind: "body", body: body.id, m: vec3(0, 0, 0) },
        origins,
      );
    } catch {
      // A frame the scene does not know: nothing to place the craft by.
      continue;
    }
    const v = velocityAgainst(craft, body);
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

/** A wanted body as the session holds it. */
interface SessionBody {
  /** Its survey's GPU coverage mask. */
  readonly mask: CoverageMask;
  /** Its survey as its pass's selection reads it, at once (R10.T16.b). */
  readonly survey: SurveyedCells;
  seed: SeedState;
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

/** No view shows the terrain's fault. */
const NO_FAULTS: ReadonlySet<ViewId> = new Set();

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
  readonly #bodies = new Map<BodyIdHex, SessionBody>();
  readonly #reports = new Map<ViewId, TerrainViewReport>();
  /** Each handed-over body's pass (R10.T16.b), from its first hand-over to its field's release. */
  readonly #passes = new Map<BodyIdHex, TerrainPass>();
  readonly #faultListeners = new Set<() => void>();
  readonly #unsubscribe: ReadonlyArray<() => void>;
  readonly #workerCount: number;
  #faults: ReadonlySet<ViewId> = NO_FAULTS;
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

  /** The bodies with a terrain pass: each handed over since its field was last taken (R10.T16.b). */
  get passBodies(): ReadonlySet<BodyIdHex> {
    return new Set(this.#passes.keys());
  }

  /**
   * The views whose label blocks show the terrain's fault, `TERRAIN_NOT_AVAILABLE`, after any
   * graphics fault (decision-r10-t16-contracts §7): every view that hands a body over while every
   * height worker is lost; the same object until it changes, for `useSyncExternalStore`.
   */
  get terrainFaults(): ReadonlySet<ViewId> {
    return this.#faults;
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
   * frame's {@link TerrainSession.step}, once a view frame.
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
  }

  /**
   * One step, once a primary frame before it draws: wants and releases the bodies' fields as every
   * view's last report judges them (T7's `follow` over {@link viewFieldCandidates}), makes or
   * releases each body's mask, detail seed and class map with its want, and steps each body's
   * pass (R10.T16.b) once over the views that hand its body over, making the pass at its first
   * hand-over.
   *
   * @remarks
   * The views' debounces take their time from their own draws (`TerrainViewFrameInput.nowMs`), so
   * the step takes none.
   */
  step(): void {
    if (this.#disposed) {
      return;
    }
    const reliefM = (body: BodyIdHex): number | null =>
      this.#store.header(body)?.realised_relief_m ?? null;
    const candidates = [...this.#reports.values()].flatMap((report) =>
      viewFieldCandidates(report, reliefM),
    );
    this.#store.follow(candidates);
    this.#followWants();
    this.#stepPasses();
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
   * A wanted body's survey for R10.T12's `SurveyLine`: the store's status, coverage and header,
   * stale while the link is down; `null` for a body not wanted.
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
      stale: coverage?.stale ?? !this.#linkUp,
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

  /** The session's memory report: the fields' copies and its GPU bytes. */
  memory(): TerrainSessionMemory {
    return {
      workerFieldBytes: this.#pool.fieldResidentBytes,
      keptFieldBytes: this.#pool.keptFieldBytes,
      gpuBytes: this.#textures.bytesByCategory(),
    };
  }

  /**
   * Tears the session down, in order: its passes (their resources, meshes and materials released,
   * their lanes ended), the class-map store, the masks (with the detail seeds' requests and the
   * session's listeners), the store (its requests cancelled and its fields released) and the pool
   * (its workers terminated). Nothing is left: no worker, request, timer, listener or GPU handle.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    for (const pass of this.#passes.values()) {
      pass.dispose();
    }
    this.#passes.clear();
    this.#faultListeners.clear();
    this.#classMaps.dispose();
    for (const unsubscribe of this.#unsubscribe) {
      unsubscribe();
    }
    for (const held of this.#bodies.values()) {
      if (held.seed.kind === "asking") {
        held.seed.cancel();
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
    const pass = this.#passes.get(change.body);
    if (pass === undefined) {
      held.mask.survey(change);
      return;
    }
    pass.setLevels(change.levels);
    const epoch = held.epoch;
    pass.fieldChanged(change, () => {
      if (!this.#disposed && this.#bodies.get(change.body) === held && held.epoch === epoch) {
        held.mask.survey(change);
        this.#flushCoverage(change.body, held, pass);
      }
    });
    this.#flushCoverage(change.body, held, pass);
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
    const pass = this.#passes.get(body);
    if (pass === undefined) {
      held.mask.update(coverage);
      return;
    }
    held.pendingCoverage = { coverage, posted: this.#pool.fieldBlocks(body)?.posted ?? 0 };
    this.#flushCoverage(body, held, pass);
  }

  /** Gives a body's mask the coverage held back, once its pass is settled and the blocks taken. */
  #flushCoverage(body: BodyIdHex, held: SessionBody, pass: TerrainPass): void {
    const pending = held.pendingCoverage;
    if (pending === null || !pass.settled) {
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
    const pass = this.#passes.get(body);
    if (pass !== undefined) {
      this.#passes.delete(body);
      pass.dispose();
    }
    held?.mask.release();
  }

  /**
   * Steps each body's pass over the views that hand it over, making a pass at a body's first
   * hand-over, and follows the fault: every view that hands a body over shows it while every height
   * worker is lost.
   */
  #stepPasses(): void {
    const handing = this.#handingViews();
    for (const [body, views] of handing) {
      const first = views[0];
      if (!this.#passes.has(body) && first !== undefined) {
        this.#makePass(body, first);
      }
    }
    for (const [body, pass] of this.#passes) {
      const views = handing.get(body) ?? [];
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
              datumGroundUnder(pass.planet.figure),
              finestPatchSizeM(pass.planet),
            );
      pass.step({ views: passViews, grounded });
    }
    const faulted = this.#pool.lostWorkers === this.#pool.workerCount;
    const faults = faulted
      ? new Set([...handing.values()].flatMap((views) => views.map(({ id }) => id)))
      : NO_FAULTS;
    if (faults.size !== this.#faults.size || [...faults].some((id) => !this.#faults.has(id))) {
      this.#faults = faults.size === 0 ? NO_FAULTS : faults;
      for (const listener of this.#faultListeners) {
        listener();
      }
    }
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
   * the wireframe's hand-over is R10.T11's).
   */
  #handingViews(): Map<BodyIdHex, HandingView[]> {
    const handing = new Map<BodyIdHex, HandingView[]>();
    for (const [id, report] of this.#reports) {
      if (report.style !== "photorealistic") {
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
   * Makes `body`'s pass at its first hand-over, on its field header's figure and the workers' level
   * table, through a lane of the pool, once its detail seed and header are held; nothing while the
   * device is lost (the next step tries again).
   */
  #makePass(body: BodyIdHex, first: HandingView): void {
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
    let pass: TerrainPass;
    try {
      pass = new TerrainPass({
        engine: this.#engine,
        setting: first.report.setting,
        planet: planetGeometry(figure, this.#pool.fieldLevels(body)),
        source: { kind: "body", body, detailSeed, weights: "baked" },
        createPool: (bake) => this.#pool.lane(bake),
        survey: held.survey,
      });
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return;
      }
      throw error;
    }
    this.#passes.set(body, pass);
    void pass.ready().catch((error: unknown) => {
      console.error(`the terrain material of ${body} failed:`, error);
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
   * detail seed's request, its class map and its mask.
   */
  #drop(body: BodyIdHex, held: SessionBody): void {
    this.#passes.get(body)?.dispose();
    this.#passes.delete(body);
    this.#bodies.delete(body);
    if (held.seed.kind === "asking") {
      held.seed.cancel();
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
   * the link: the session's one fetch of the hooks, which R08.T10.a's shares.
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
        const seed = bodyDetailSeed(outcome.response.record);
        if (seed === null) {
          held.seed = { kind: "none" };
        } else {
          this.#seedHeld(body, held, seed);
        }
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
}
