/**
 * The smoke harness's frame check (plan R10, R10.T16.c, `R10.T16 the frame`): a photorealistic
 * frame with the Moon-like fixture's body handed over to its terrain, drawn as `VIEW` draws it, on
 * the real device; the holder, for the whole of R10.T16, of "no pixel drawn by both surfaces on the
 * datum" (decision-r10-t16-contracts §6).
 *
 * @remarks
 * The world is the Moon-like fixture's first survey, taken by a pool of two height workers made
 * without bake settings, as the terrain session's is; its blocks feed the body's survey, which
 * selection reads, and its GPU coverage mask. Two lit passes bake through lanes of the pool: one as
 * baked, and one whose every bake is laid on the datum (`onTheDatum`), so that the two surfaces
 * coincide and no relief stands one in front of the other. The camera is the survey-edge check's,
 * 30 km over the first survey's +i edge about 60° below the horizon (`poseOver`), the body
 * handed over by its relief at its limb. Each frame is made through the live path: the plan's body
 * gives each pass's view (`terrainViewOf`), its disc record the lights (`terrainLightingOf`), the
 * view's `LiveFrameTerrain` the draws and the mask, given only once the view's draw set leaves
 * nothing missing. It checks:
 *
 * - the frame's bodies pass, the reference surface then the terrain, as `PhotorealRenderer` draws
 *   it, into a scene target: every texel finite and every pixel drawn by one or the other;
 * - on the datum, each surface alone: no pixel drawn by both, and each drawing its part;
 * - the whole frame through `PhotorealRenderer` onto a canvas, its plan handing the body over, with
 *   two marks in its symbology: one 15 km under the surveyed ground, which the terrain's depth in
 *   the symbology's pass hides, and one 2 km over it, which stands; drawn again with no terrain
 *   depth, both stand;
 * - the whole frame again, its symbology the body's occluder sphere and two lines of its own on the
 *   datum, as its graticule is: the one over the surveyed ground gives way to the terrain, and the
 *   one beyond the survey's edge stands (R10.T16.f); drawn again with no datum giving way, as the
 *   overlay was built before R10.T16.f, both stand;
 * - R10.T16.c's grazing fix, over a third pass whose every bake lies wholly on the datum
 *   (`whollyOnTheDatum`), seen grazing it: from 10 km up at 2°, a mark 1 km behind the ground,
 *   which the terrain's depth hides (its old slope term, some 16 km there, showed it); and from
 *   2 km up at 2°, 10° and 60°, a line on the ground drawn as the survey edge is, pulled to the
 *   ground, which draws whole, where the same line not pulled loses its nearer half;
 * - and its per-fragment pull, over a fourth pass wholly on the datum but for a ridge across the
 *   view (`ridged`): from 10 km up at 2°, a line on the datum drawn as the survey edge is, behind
 *   the ridge by less than the 3 px of the datum's depth slope every fragment was once pulled by,
 *   draws no pixel.
 */

import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import {
  type BodyFrameOptions,
  type BodyFramePlan,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../view/bodies/draw";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  project,
  viewRotation4,
} from "../view/camera/projection";
import {
  DEPTH_PYRAMID_KERNEL,
  DepthPyramid,
  type FrameOcclusion,
  pyramidProjectionOf,
} from "../view/clusters/hzb";
import type { ClusterPassStats } from "../view/clusters/stats";
import { lookAlong, rotate } from "../view/camera/quaternion";
import { viewId } from "../view/camera/state";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import { occluderRadius } from "../view/depth/depth";
import type { KernelPair } from "../view/engine/kernels";
import type { DrawItem, RenderEngine, RenderView } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { overlaySubmission } from "../view/photoreal/overlay";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { type PhotorealOverlay, PhotorealRenderer } from "../view/photoreal/renderer";
import {
  GEOMETRY_BEFORE_R14,
  type GeometrySettings,
  SETTINGS,
  TERRAIN_SETTINGS,
} from "../view/quality/qualitySetting";
import type { CapturedImage } from "./atmosphere";
import { captureOf, captureScale, isTerrain, maskOf, stopsApart } from "./leverCaptures";
import { AU_M } from "../view/scenes/kept";
import { CoverageMask } from "../view/terrain/coverage";
import {
  type FrameTerrainSource,
  LiveFrameTerrain,
  terrainViewOf,
} from "../view/terrain/frameTerrain";
import type { FrameTerrain } from "../view/photoreal/renderer";
import { handoverTauPx, limbDistanceM, pixelAngleRad } from "../view/terrain/handover";
import { uvToSt, vertexSpacing, xyzToFaceUv } from "../view/terrain/cube";
import { type PatchKey, sameKey } from "../view/terrain/patchKey";
import { planetGeometry } from "../view/terrain/planet";
import { SurveyedCells } from "../view/terrain/surveyLookup";
import {
  TerrainPass,
  type TerrainDrawnPatches,
  type TerrainGeometryCounts,
  type TerrainPool,
} from "../view/terrain/terrainPass";
import {
  CLUSTER_LIT_LATE_PASS,
  CLUSTER_LIT_PASS,
  LIT_INTERIOR_INDICES,
  LIT_PATCH_INDICES,
} from "../view/terrain/gpu/resources";
import { PATCH_VERTICES_PER_SIDE } from "../view/terrain/slotLayout";
import {
  finestPatchSizeM,
  type GroundContact,
  heldGroundUnder,
  heldRadiusM,
} from "../view/terrain/grounded";
import type { BakedPatch } from "../view/terrain/workers/messages";
import { HeightWorkerPool, type HeightWorkerLane } from "../view/terrain/workers/pool";
import {
  emptyDrawList,
  type LineBatch,
  viewStrokesAt,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { WireframeRenderer, WIREFRAME_MATERIALS } from "../view/wireframe/submit";
import { moonLikeFixture } from "../test/moonLikeFixture";
import { addCanvas } from "./frames";
import { bytesDigest, type Checks, halfTexels, pause } from "./harness";
import { MOON_LIKE } from "./litTerrain";
import {
  bodyAt,
  datumAt,
  drawn,
  EXPOSURE,
  onTheDatum,
  OVER,
  type Pose,
  poseOver,
  VIEWPORT,
} from "./surveyEdge";
import { BROKEN_CULL_KERNEL, type SmokeFixture } from "./catalogue";
import { smokeGeometry } from "./geometry";
import { LaneHold, type LockstepStep, settleInLockstep } from "./lockstep";

/** The passes' cache budget, 32 MiB: a few hundred lit slots, as the survey-edge check takes. */
const CACHE_BYTES = 2 ** 25;

/** How long the workers may take over the field, and a pass over its streaming, milliseconds. */
const FIELD_TIMEOUT_MS = 60_000;
const STREAM_TIMEOUT_MS = 90_000;

/**
 * The longest the occlusion's ridge check streams its pass on past its settle (plan R14, R14.T3.d),
 * until nothing stands in; on the pod it rests well within it.
 */
const RIDGE_STREAM_MS = 30_000;

/**
 * How long R14.T3.c's pass about a grounded craft may stream, milliseconds: its forced region to the
 * finest level, the part's alone (part 16).
 */
const CRAFT_TIMEOUT_MS = 150_000;

/** 70° and 8°, rad: R14.T3.c's view of a grounded craft, its depression and its field. */
const RAD_70 = (70 * Math.PI) / 180;
const RAD_8 = (8 * Math.PI) / 180;

/** The view's name, as the session knows `VIEW`'s primary. */
const VIEW = viewId("view");

/** The Moon-like figure with no pole known, as the passes' selection reads it. */
const MOON_LIKE_FIGURE = { ...MOON_LIKE, pole: null };

/** The colour of the check's marks, as `readTokens` would read a token: magenta, which no ground is. */
const MARK = "#ff00ff";

/** The tokens the overlay takes: the marks' casing colour. */
const TOKENS = { surface0: "#000000" };

/** The patch past the survey's +i edge from {@link OVER}: column 48, in its unsurveyed cell 24. */
const BEYOND: PatchKey = { ...OVER, i: OVER.i + 1 };

/**
 * A bake laid wholly on the datum (R10.T16.c's grazing checks): {@link onTheDatum}'s, its origin,
 * from which the face-differences path places each vertex, moved onto the datum's point at vertex
 * (32, 32), and its height range the datum's, by which the pass culls it, so that its ground is the
 * datum's to the millimetre. `onTheDatum`'s patches stand each at its origin's height, a few hundred
 * metres off the datum here, as its checks' poses are set for.
 */
function whollyOnTheDatum(bake: BakedPatch): BakedPatch {
  return { ...onTheDatum(bake), originM: datumAt(bake.key, 32, 32).p, heightRangeM: [0, 0] };
}

/** The ridge's height at its crest, m (R10.T16.c's ridge check, {@link ridged}). */
const RIDGE_HEIGHT_M = 1_500;

/** How far either side of its crest the ridge falls to the datum, m. */
const RIDGE_HALF_WIDTH_M = 2_500;

/** How much nearer the camera's foot than the edge behind it the ridge's crest stands, m. */
const RIDGE_LEAD_M = 11_700;

/**
 * Bakes laid wholly on the datum ({@link whollyOnTheDatum}) but for a ridge across `view`: its
 * crest {@link RIDGE_LEAD_M} nearer the camera's foot along the surface than the body-fixed point
 * `edgeM`, {@link RIDGE_HEIGHT_M} high there and falling as cos² to the datum
 * {@link RIDGE_HALF_WIDTH_M} either side, each vertex's own and morph heights alike. Their cluster
 * errors, measured on the bake's own ground, are dropped, so that no cluster of the ridge is drawn
 * coarser on them (unmeasured records are drawn whole).
 */
function ridged(view: Pose, edgeM: Vec3): (bake: BakedPatch) => BakedPatch {
  const foot = normalise(view.cameraM);
  const radiusM = norm(edgeM);
  const alongM = (p: Vec3): number => Math.acos(Math.min(1, dot(normalise(p), foot))) * radiusM;
  const crestM = alongM(edgeM) - RIDGE_LEAD_M;
  return (bake) => {
    const { clusterErrors: _clusterErrors, ...laid } = whollyOnTheDatum(bake);
    const heights = new Float32Array(bake.heights.length);
    let topM = 0;
    for (let y = 0; y < PATCH_VERTICES_PER_SIDE; y += 1) {
      for (let x = 0; x < PATCH_VERTICES_PER_SIDE; x += 1) {
        const offM = alongM(datumAt(bake.key, x, y).p) - crestM;
        const heightM =
          Math.abs(offM) < RIDGE_HALF_WIDTH_M
            ? RIDGE_HEIGHT_M * Math.cos(((Math.PI / 2) * offM) / RIDGE_HALF_WIDTH_M) ** 2
            : 0;
        const n = y * PATCH_VERTICES_PER_SIDE + x;
        heights[2 * n] = heightM;
        heights[2 * n + 1] = heightM;
        topM = Math.max(topM, heightM);
      }
    }
    return {
      ...laid,
      heights,
      heightRangeM: [0, topM],
      boundingRadiusM: bake.boundingRadiusM + topM,
    };
  };
}

/** A lane of the pool whose bakes are laid on the datum by `lay` before the pass takes them. */
function onTheDatumLane(
  lane: HeightWorkerLane,
  lay: (bake: BakedPatch) => BakedPatch,
): TerrainPool {
  return {
    reprioritise: (demand) => {
      lane.reprioritise(demand);
    },
    onBaked: (cb) =>
      lane.onBaked((bake) => {
        cb(lay(bake));
      }),
    onFailed: (cb) => lane.onFailed(cb),
    terminate: () => {
      lane.terminate();
    },
    get unavailable(): boolean {
      return lane.unavailable;
    },
  };
}

/** Waits for `done`, polling every 20 ms, up to `timeoutMs`; whether it came. */
async function until(done: () => boolean, timeoutMs: number): Promise<boolean> {
  const started = performance.now();
  while (!done()) {
    if (performance.now() - started > timeoutMs) {
      return false;
    }
    // The workers and the engine answer on the event loop.
    // oxlint-disable-next-line no-await-in-loop
    await pause(20);
  }
  return true;
}

/** The body as the frame lights it: handed over by a relief of four times τ at its limb. */
function bodyOf(pose: Pose): LitBodyInput {
  const centreM = scale(pose.cameraM, -1);
  const limbM = limbDistanceM(
    Math.hypot(centreM.x, centreM.y, centreM.z),
    MOON_LIKE.equatorialRadiusM,
  );
  return {
    id: "5e5e000000000000.0003",
    centreM,
    figure: { ...MOON_LIKE, pole: { x: 0, y: 0, z: 1 } },
    photometry: PROVISIONAL_PHOTOMETRY,
    rotation: IDENTITY_ROTATION,
    handoverReliefM:
      4 * handoverTauPx("photorealistic", "low") * limbM * pixelAngleRad(pose.camera, VIEWPORT),
    lighting: undefined,
  };
}

/** A frame's lit bodies options, as the low setting's view takes them. */
function optionsOf(pose: Pose): BodyFrameOptions {
  return {
    camera: pose.camera,
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    exposureScale: EXPOSURE,
    annuli: DISC_ANNULI_HIGH,
    planetshine: PLANETSHINE_SOURCES_HIGH,
    setting: "low",
  };
}

/** One frame of `pass`'s view through the live path: its plan, terrain draws and mask. */
interface LiveFrame {
  readonly plan: BodyFramePlan;
  readonly body: LitBodyInput;
  readonly terrain: LiveFrameTerrain;
  readonly terrainDraws: ReturnType<LiveFrameTerrain["draws"]>;
}

/** The patches the last frame of each pass drew (plan R14, R14.T1.a), by pass. */
const drawnPatches = new WeakMap<TerrainPass, TerrainDrawnPatches>();

/** The hold on each pass's lanes, which {@link settled} releases and holds again (`lockstep.ts`). */
const laneHolds = new WeakMap<TerrainPass, LaneHold>();

/** The view's frame terrain over `pass` and the body's mask, `VIEW`'s session's seams. */
function sourceOf(pass: TerrainPass, mask: CoverageMask): FrameTerrainSource {
  return {
    drawTerrain: (view, _body, input) => {
      const frame = pass.drawFor(view, input);
      drawnPatches.set(pass, frame.patches);
      return frame;
    },
    coverageMask: () => mask.texture,
    coverageWhole: () => mask.whole,
    drawLateTerrain: (view) => pass.lateDrawFor(view),
    pass: () => pass,
  };
}

/** The pixels at which two frames' texels differ, and the largest difference of any channel. */
function differingTexels(
  a: Float32Array,
  b: Float32Array,
): { readonly pixels: number; readonly largest: number } {
  let pixels = 0;
  let largest = 0;
  for (let p = 0; p + 3 < Math.min(a.length, b.length); p += 4) {
    let differs = false;
    for (let c = 0; c < 4; c += 1) {
      const d = Math.abs((a[p + c] ?? 0) - (b[p + c] ?? 0));
      if (d > 0 || Number.isNaN(d)) {
        differs = true;
        largest = Math.max(largest, Number.isNaN(d) ? Infinity : d);
      }
    }
    pixels += differs ? 1 : 0;
  }
  return { pixels, largest };
}

/**
 * The pixels of a terrain-alone frame `before` draws inside its terrain, the pixel and its eight
 * neighbours drawn, that `after` leaves undrawn (plan R14, R14.T3.c): where a crack between its
 * clusters shows what lies behind.
 */
function cracksOf(before: Float32Array, after: Float32Array): number {
  const { widthPx, heightPx } = VIEWPORT;
  let cracks = 0;
  for (let y = 1; y + 1 < heightPx; y += 1) {
    for (let x = 1; x + 1 < widthPx; x += 1) {
      const p = y * widthPx + x;
      if (isTerrain(after, p)) {
        continue;
      }
      let inside = true;
      for (let dy = -1; dy <= 1 && inside; dy += 1) {
        for (let dx = -1; dx <= 1 && inside; dx += 1) {
          inside = isTerrain(before, p + dy * widthPx + dx);
        }
      }
      cracks += inside ? 1 : 0;
    }
  }
  return cracks;
}

/** Pass `pass`'s clusters drawn in `stats`, or `null` without them (plan R14, R14.T3.d). */
function litOf(stats: ReadonlyArray<ClusterPassStats> | null, pass: number): number | null {
  return stats?.[pass]?.clusters ?? null;
}

/** No patches drawn, where a frame gave no counts. */
const NO_DRAWN_PATCHES: TerrainDrawnPatches = { interior: 0, edge: 0, fins: 0 };

/**
 * Steps `pass` from `pose`, with `grounded`'s contacts, and makes the frame, as `VIEW`'s loop and
 * drawer do, with `occlusion` where its renderer runs the two-pass occlusion (plan R14, R14.T3.d).
 */
function liveFrame(
  pass: TerrainPass,
  mask: CoverageMask,
  pose: Pose,
  nowMs: number,
  grounded: ReadonlyArray<GroundContact> = [],
  occlusion?: FrameOcclusion,
): LiveFrame {
  const frame = steppedFrame(pass, mask, pose, nowMs, grounded);
  return { ...frame, terrainDraws: frame.terrain.draws(frame.plan, [frame.body], occlusion) };
}

/**
 * {@link liveFrame}'s frame before its terrain draws, for a renderer that draws them itself with
 * its own occlusion (plan R14, R14.T3.d).
 */
function steppedFrame(
  pass: TerrainPass,
  mask: CoverageMask,
  pose: Pose,
  nowMs: number,
  grounded: ReadonlyArray<GroundContact> = [],
): Omit<LiveFrame, "terrainDraws"> {
  const base = bodyOf(pose);
  const view = terrainViewOf(base, pose.camera, VIEWPORT);
  if (view === null) {
    throw new Error("the body turns");
  }
  pass.step({
    views: [{ id: VIEW, view, tauPx: handoverTauPx("photorealistic", "low"), weight: 1 }],
    grounded,
  });
  const terrain = new LiveFrameTerrain(sourceOf(pass, mask), {
    view: VIEW,
    camera: pose.camera,
    presentedPx: VIEWPORT,
    exposureScale: EXPOSURE,
    occluderSlopePx: emptyDrawList(viewStrokesAt(1)).occluderSlopePx,
    nowMs,
  });
  const coverage = terrain.surveyCoverage(base.id);
  const body: LitBodyInput =
    coverage === null
      ? base
      : {
          ...base,
          surveyCoverage: {
            mask: coverage,
            rotation: IDENTITY_ROTATION,
            whole: terrain.surveyedWhole(base.id),
          },
        };
  const plan = planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(body.centreM, scale(pose.sun, AU_M)) }],
    optionsOf(pose),
    new Map([[body.id, "disc"]]),
  );
  return { plan, body, terrain };
}

/**
 * A frame drawn as the renderer draws it with the two-pass occlusion (plan R14, R14.T3.d): `early`,
 * then `pyramid` built from that raster's depth, then `late()`'s draws over it, loading its colour
 * and depth; read back.
 */
async function drawnOccluded(
  engine: RenderEngine,
  pose: Pose,
  early: ReadonlyArray<DrawItem>,
  pyramid: DepthPyramid,
  late: () => ReadonlyArray<DrawItem>,
): Promise<Float32Array> {
  const target = createSceneTarget(engine, "smoke occluded frame", VIEWPORT);
  try {
    const depth = target.depth;
    if (depth === null) {
      throw new Error("the scene target has no depth");
    }
    const frame = {
      viewRotation: viewRotation4(pose.camera.orientation),
      projection: perspectiveReversedInfinite(
        pose.camera.fovXRad,
        VIEWPORT.widthPx / VIEWPORT.heightPx,
        NEAR_PLANE_M,
      ),
      postProcesses: [],
    };
    target.render({ ...frame, label: PHOTOREAL_PASS_LABELS.bodies, draws: early });
    pyramid.build(depth, pyramidProjectionOf(pose.camera.fovXRad, VIEWPORT));
    target.render({
      ...frame,
      label: PHOTOREAL_PASS_LABELS.bodiesLate,
      draws: late(),
      colourLoad: "load",
    });
    return halfTexels(await engine.readTexture(target.colour));
  } finally {
    target.dispose();
  }
}

/** A frame through the photorealistic renderer (plan R14, R14.T3.d), its scene read back. */
interface SceneFrame {
  readonly colour: Float32Array;
  readonly depth: Float32Array;
  /** Whether its terrain drew a late raster. */
  readonly late: boolean;
}

/** What a frame through the renderer is drawn with: its renderer and canvas, a pass and its view. */
interface SceneOf {
  readonly engine: RenderEngine;
  readonly photoreal: PhotorealRenderer;
  readonly canvas: RenderView;
  readonly pass: TerrainPass;
  readonly mask: CoverageMask;
  readonly view: Pose;
}

/**
 * One frame of `of`'s view through its renderer at `geometry`, as `VIEW` draws it, with no
 * symbology, held back where `present` is `false` (a cut): its scene's colour and depth.
 */
async function sceneFrame(
  of: SceneOf,
  geometry: GeometrySettings,
  present: boolean,
): Promise<SceneFrame> {
  const { engine, photoreal, canvas, pass, mask, view } = of;
  const frame = steppedFrame(pass, mask, view, performance.now());
  let late = false;
  const terrain: FrameTerrain = {
    surveyCoverage: (body) => frame.terrain.surveyCoverage(body),
    surveyedWhole: (body) => frame.terrain.surveyedWhole(body),
    draws: (...args) => frame.terrain.draws(...args),
    symbologyDepth: () => frame.terrain.symbologyDepth(),
    sceneDepth: () => frame.terrain.sceneDepth(),
    lateDraws: () => {
      const draws = frame.terrain.lateDraws();
      late = draws.length > 0;
      return draws;
    },
  };
  photoreal.render(canvas, {
    camera: view.camera,
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    role: "eye",
    setting: "low",
    geometry,
    exposureScale: EXPOSURE,
    sky: [],
    starSprites: [],
    hostDraws: new Map(),
    glareSources: [],
    lights: [{ disc: sunLikeHostDisc(), centreM: add(frame.body.centreM, scale(view.sun, AU_M)) }],
    bodies: [frame.body],
    depthWriters: [],
    previousRegimes: new Map([[frame.body.id, "terrain"]]),
    terrain,
    overlay: null,
    meter: null,
    present,
  });
  const colours = photoreal.frameColours;
  const depth = colours?.scene.depth ?? null;
  if (colours === null || depth === null) {
    throw new Error("the renderer drew no scene");
  }
  const [colour, depthBytes] = await Promise.all([
    engine.readTexture(colours.scene.colour),
    engine.readTexture(depth),
  ]);
  return { colour: halfTexels(colour), depth: new Float32Array(depthBytes), late };
}

/** A frame and its pass's counters: the last of nine drawn alike. */
interface CountedFrame {
  readonly frame: SceneFrame;
  readonly stats: ReadonlyArray<ClusterPassStats> | null;
}

/**
 * `of`'s view through the renderer in one raster and with the two-pass occlusion (plan R14,
 * R14.T3.d), from the same selection and residents, at `geometry` but its `occlusion`: with it, at
 * frames that each drop the view's visibility (held back, a cut) and at frames that keep it; each
 * the last of nine, the counters on, so that its counts are its own.
 */
async function occlusionRun(
  of: SceneOf,
  geometry: GeometrySettings,
): Promise<{
  readonly one: CountedFrame;
  readonly cut: CountedFrame;
  readonly kept: CountedFrame;
}> {
  const { pass } = of;
  const counted = async (at: GeometrySettings, present: boolean): Promise<CountedFrame> => {
    let frame = await sceneFrame(of, at, present);
    for (let round = 0; round < 8; round += 1) {
      // Each frame reads the counters that some frames before it answered: the last of nine.
      // oxlint-disable-next-line no-await-in-loop
      frame = await sceneFrame(of, at, present);
    }
    return { frame, stats: pass.clusterStats(VIEW) };
  };
  const oneRaster: GeometrySettings = { ...geometry, occlusion: "off" };
  const occluded: GeometrySettings = { ...geometry, occlusion: "hzb" };
  pass.measureClusters(true);
  try {
    pass.setGeometry(oneRaster);
    const one = await counted(oneRaster, true);
    pass.setGeometry(occluded);
    const cut = await counted(occluded, false);
    const kept = await counted(occluded, true);
    return { one, cut, kept };
  } finally {
    pass.setGeometry(geometry);
    pass.measureClusters(false);
  }
}

/**
 * How {@link occlusionRun}'s frames with the occlusion compare with its one raster's (plan R14,
 * R14.T3.d): whether each one's colour differs at no pixel (no tie pixel) and its depth is the same
 * bytes, the held-back frames drawing a late raster; the clusters the pyramid refused at the frames
 * that keep the visibility; and the lit pass's clusters by pass.
 */
function occlusionSummary(run: Awaited<ReturnType<typeof occlusionRun>>): {
  readonly same: boolean;
  readonly refused: number | null;
  readonly text: string;
} {
  const { one, cut, kept } = run;
  const oneLit = litOf(one.stats, CLUSTER_LIT_PASS);
  const passes = (
    counted: CountedFrame,
  ): readonly [number | null, number | null, number | null] => {
    const early = litOf(counted.stats, CLUSTER_LIT_PASS);
    const late = litOf(counted.stats, CLUSTER_LIT_LATE_PASS);
    return [
      early,
      late,
      oneLit === null || early === null || late === null ? null : oneLit - early - late,
    ];
  };
  const compared = [cut, kept].map(({ frame }) => ({
    ties: differingTexels(one.frame.colour, frame.colour),
    depth: bytesDigest(frame.depth) === bytesDigest(one.frame.depth),
    late: frame.late,
  }));
  const [cutEarly, cutLate, cutRefused] = passes(cut);
  const [keptEarly, keptLate, refused] = passes(kept);
  const same =
    compared.every(({ ties, depth }) => ties.pixels === 0 && depth) && compared[0]?.late === true;
  const text = `the lit pass's clusters ${String(oneLit)} in one raster; with the occlusion, the visibility dropped: ${String(cutEarly)} early, ${String(cutLate)} late, ${String(cutRefused)} refused; kept: ${String(keptEarly)} early, ${String(keptLate)} late, ${String(refused)} refused; ${compared
    .map(
      ({ ties, depth, late }, n) =>
        `${n === 0 ? "dropped" : "kept"}: a late raster ${String(late)}, tie pixels ${String(ties.pixels)}, the largest difference ${ties.largest.toExponential(3)}, the depth the same bytes ${String(depth)}`,
    )
    .join(
      "; ",
    )}; digests ${bytesDigest(one.frame.colour)} in one raster, ${bytesDigest(cut.frame.colour)} and ${bytesDigest(kept.frame.colour)} with the occlusion`;
  return { same, refused, text };
}

/** The patch of `level` whose square holds the body-fixed point `p` on or near the datum, p = M·d. */
function patchHolding(p: Vec3, level: number): PatchKey {
  const { face, u, v } = xyzToFaceUv([
    p.x / MOON_LIKE.equatorialRadiusM,
    p.y / MOON_LIKE.equatorialRadiusM,
    p.z / MOON_LIKE.polarRadiusM,
  ]);
  const n = 2 ** level;
  const index = (w: number): number => Math.min(n - 1, Math.max(0, Math.floor(uvToSt(w) * n)));
  return { face, level, i: index(u), j: index(v) };
}

/** The most a stand-in's chords may sag below the datum where a check reads it, m. */
const STAND_IN_SAG_M = 1;

/** The most a patch of `level`'s chords sag below the datum, m: its widest quad's s² ÷ 8R. */
function chordSagM(level: number): number {
  const { maxM } = vertexSpacing(MOON_LIKE.equatorialRadiusM, level);
  return (maxM * maxM) / (8 * MOON_LIKE.polarRadiusM);
}

/**
 * Streams `pass`, with `grounded`'s contacts, until its view's frame gives the mask and draws its
 * terrain whole, or time runs out: every patch its own bake; or, given the body-fixed points a check reads (`near`), every
 * patch drawn and none standing in whose square holds one of them and whose chords sag more than
 * {@link STAND_IN_SAG_M} below the datum (154 m at level 0, 0.6 m at level 4); and, either way, no
 * draft waiting for its refined normals (R10.T6.a), so that the frames a check compares shade alike.
 * The grazing views'
 * ground out to the horizon goes on standing in over the survey's margin, which the checks never
 * read, and their fine patches at the points stream for longer than the part allows.
 *
 * Each step waits for every bake `pool` was asked for to land, and the frame is the first that a
 * step leaves as it was, its bands resting (`lockstep.ts`), so that it is the same whatever the
 * workers' pace; and the pass's lanes are held from then until its next settle, so that the frames
 * a check draws after it draw what it settled on.
 */
async function settled(
  pass: TerrainPass,
  pool: HeightWorkerPool,
  mask: CoverageMask,
  pose: Pose,
  near: ReadonlyArray<Vec3> | null = null,
  grounded: ReadonlyArray<GroundContact> = [],
  timeoutMs = STREAM_TIMEOUT_MS,
): Promise<{ readonly done: boolean; readonly frame: LiveFrame; readonly state: string }> {
  let state = "";
  const step = (): LockstepStep<LiveFrame> => {
    const frame = liveFrame(pass, mask, pose, performance.now(), grounded);
    const drawSet = pass.lastStep?.drawSet;
    const role = frame.plan.meshes[0]?.role;
    const standsInNear = (): boolean =>
      (drawSet?.patches ?? []).some(
        ({ patch, standIn }) =>
          standIn &&
          chordSagM(patch.key.level) > STAND_IN_SAG_M &&
          (near ?? []).some((p) => sameKey(patchHolding(p, patch.key.level), patch.key)),
      );
    const done =
      role?.kind === "handed-over" &&
      role.coverage !== null &&
      frame.terrainDraws.length >= 1 &&
      drawSet !== undefined &&
      drawSet.missing === 0 &&
      pass.refining === 0 &&
      (near === null ? drawSet.standingIn === 0 : !standsInNear());
    state = `role ${String(role?.kind)}, ${String(frame.terrainDraws.length)} terrain draws, ${String(drawSet?.missing)} missing, ${String(drawSet?.standingIn)} standing in, ${String(pass.refining)} refining${near === null ? "" : `, near ${String(standsInNear())}`}`;
    return { done, frame, drawSet, resting: pass.bandsAtTarget(VIEW) };
  };
  const streamed = (): boolean =>
    pool.queuedCount === 0 &&
    pool.inFlightCount === 0 &&
    pool.waitingCount === 0 &&
    pass.refining === 0;
  const hold = laneHolds.get(pass);
  hold?.release();
  const { done, frame } = await settleInLockstep(step, streamed, timeoutMs);
  hold?.hold();
  return { done, frame, state };
}

/** Whether two frames show the same marks, mark by mark. */
function sameShown(a: ReadonlyArray<boolean>, b: ReadonlyArray<boolean>): boolean {
  return a.length === b.length && a.every((shown, n) => shown === b[n]);
}

/** The midpoint of a mark's two ends. */
function midpoint(pair: readonly [Vec3, Vec3]): Vec3 {
  return scale(add(pair[0], pair[1]), 0.5);
}

/** The number of `texels`' pixels drawn by a body. */
function bodyPixels(texels: Float32Array, pixels: number): number {
  let count = 0;
  for (let p = 0; p < pixels; p += 1) {
    count += bodyAt(texels, p) ? 1 : 0;
  }
  return count;
}

/** A view-space mark from `a` to `b`, m from the camera, in the check's colour. */
function markBatch(id: string, a: Vec3, b: Vec3): LineBatch {
  return {
    id,
    space: "view",
    originF32: new Float32Array(3),
    segments: Float32Array.from([a.x, a.y, a.z, b.x, b.y, b.z]),
    token: "text",
    colour: MARK,
    widthPx: 2,
    casingWidthPx: 2,
    casingColour: "#000000",
    dash: null,
  };
}

/**
 * Whether the canvas read-back's pixel at byte `n` is the marks' magenta: red and blue near full
 * and green near none, in either channel order and either encoding the canvas has.
 */
function isMarked(read: Float32Array | Uint8Array, n: number): boolean {
  const full = read instanceof Uint8Array ? 255 : 1;
  const [r = 0, g = full, b = 0] = [read[n], read[n + 1], read[n + 2]];
  return r > 0.8 * full && g < 0.3 * full && b > 0.8 * full;
}

/** Whether any pixel within one of `at` on the canvas read-back is the marks' magenta. */
function markedNear(
  read: Float32Array | Uint8Array,
  at: { readonly xPx: number; readonly yPx: number },
): boolean {
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      const x = Math.floor(at.xPx) + dx;
      const y = Math.floor(at.yPx) + dy;
      if (x < 0 || y < 0 || x >= VIEWPORT.widthPx || y >= VIEWPORT.heightPx) {
        continue;
      }
      if (isMarked(read, 4 * (y * VIEWPORT.widthPx + x))) {
        return true;
      }
    }
  }
  return false;
}

/** How many pixels of each row of the canvas read-back, top first, are the marks' magenta. */
function markedRows(read: Float32Array | Uint8Array): number[] {
  return Array.from({ length: VIEWPORT.heightPx }, (_, y) => {
    let count = 0;
    for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
      count += isMarked(read, 4 * (y * VIEWPORT.widthPx + x)) ? 1 : 0;
    }
    return count;
  });
}

/** The row the grazing checks read, just above the view's centre, where each pose's datum point is. */
const GRAZE_ROW = 23;

/**
 * Where the ray of `view` through pixel (`xPx`, `yPx`) meets the datum, from the camera, m: on the
 * sphere of the datum's radius at {@link OVER}'s centre, within metres of the spheroid near it; or
 * on the sphere of `radiusM`.
 */
function datumOnRay(
  view: Pose,
  xPx: number,
  yPx: number,
  radiusM = norm(datumAt(OVER, 32, 32).p),
): Vec3 {
  const s = 1 / Math.tan(view.camera.fovXRad / 2);
  const aspect = VIEWPORT.widthPx / VIEWPORT.heightPx;
  const ray = normalise(
    rotate(
      view.camera.orientation,
      vec3(
        ((2 * xPx) / VIEWPORT.widthPx - 1) / s,
        (1 - (2 * yPx) / VIEWPORT.heightPx) / (s * aspect),
        -1,
      ),
    ),
  );
  // |c + t r| = R for the camera c from the centre: the nearer root.
  const b = dot(view.cameraM, ray);
  const c = dot(view.cameraM, view.cameraM) - radiusM * radiusM;
  return scale(ray, -b - Math.sqrt(b * b - c));
}

/**
 * A grazing view of the datum at {@link OVER}'s centre (R10.T16.c's grazing fix): the camera
 * `heightM` over the datum, towards −j along the survey's rows, whose ray to the point meets the
 * datum there `elevationRad` below its horizontal, the body's curvature included, looking along it
 * with the datum's normal there up, its field `fovXRad` across; or at vertex (`x`, `y`) of OVER.
 */
function grazingPose(elevationRad: number, heightM: number, fovXRad: number, x = 32, y = 32): Pose {
  const { p, nu } = datumAt(OVER, x, y);
  const alongJ = sub(datumAt(OVER, 32, 64).p, datumAt(OVER, 32, 0).p);
  const j = normalise(sub(alongJ, scale(nu, dot(alongJ, nu))));
  const radiusM = norm(p);
  const sin = Math.sin(elevationRad);
  // The distance along the ray at which it stands `heightM` over a sphere of the point's radius.
  const rangeM =
    -radiusM * sin +
    Math.sqrt(radiusM * radiusM * sin * sin + heightM * heightM + 2 * radiusM * heightM);
  const fromPoint = add(scale(j, -Math.cos(elevationRad)), scale(nu, sin));
  return {
    cameraM: add(p, scale(fromPoint, rangeM)),
    camera: { orientation: lookAlong(scale(fromPoint, -1), nu), fovXRad },
    sun: poseOver().sun,
  };
}

/**
 * A view of the body-fixed point `target`, whose datum's normal is `nu`, from `rangeM` away along a
 * ray `elevationRad` below its horizontal towards −j of {@link OVER} (as {@link grazingPose} looks),
 * its field `fovXRad` across.
 */
function poseLookingAt(
  target: Vec3,
  nu: Vec3,
  elevationRad: number,
  rangeM: number,
  fovXRad: number,
): Pose {
  const alongJ = sub(datumAt(OVER, 32, 64).p, datumAt(OVER, 32, 0).p);
  const j = normalise(sub(alongJ, scale(nu, dot(alongJ, nu))));
  const fromPoint = add(scale(j, -Math.cos(elevationRad)), scale(nu, Math.sin(elevationRad)));
  return {
    cameraM: add(target, scale(fromPoint, rangeM)),
    camera: { orientation: lookAlong(scale(fromPoint, -1), nu), fovXRad },
    sun: poseOver().sun,
  };
}

/**
 * R10.T16.c's frame: see the module's remarks. With `captures`, R14.T1.a's relief frame by the way
 * back and on the cluster path, at one exposure, and its terrain's mask, for a person to look at and
 * for `scripts/compareCaptures.mjs`.
 */
export async function checkTerrainFrame(
  engine: RenderEngine,
  checks: Checks,
  captures = false,
  fixture: SmokeFixture = "none",
): Promise<CapturedImage[]> {
  const images: CapturedImage[] = [];
  const field = moonLikeFixture("first");
  const pool = new HeightWorkerPool({
    workers: 2,
    createWorker: () =>
      new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
        type: "module",
      }),
  });
  const survey = new SurveyedCells();
  const mask = new CoverageMask(engine, "terrain frame smoke: coverage mask");
  const offChange = pool.onFieldChange((change) => {
    survey.add(change);
    mask.survey(change);
  });
  const passes: TerrainPass[] = [];
  const makePass = async (
    lay: ((bake: BakedPatch) => BakedPatch) | null,
    clusterCullKernel?: KernelPair,
  ): Promise<TerrainPass> => {
    const hold = new LaneHold();
    const pass = new TerrainPass({
      engine,
      setting: "low",
      terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
      geometry: smokeGeometry("low"),
      planet: planetGeometry(MOON_LIKE_FIGURE, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      createPool: (bake) =>
        hold.wrap(lay === null ? pool.lane(bake) : onTheDatumLane(pool.lane(bake), lay)),
      survey,
      ...(clusterCullKernel === undefined ? {} : { clusterCullKernel }),
    });
    // The harness compares frames drawn at different times, byte for byte: the time morph's
    // changes cut at their first draw (R10.T16.i), as under reduced motion.
    pass.setReducedMotion(true);
    laneHolds.set(pass, hold);
    passes.push(pass);
    await pass.ready();
    return pass;
  };
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  const photoreal = new PhotorealRenderer(engine, "terrain frame smoke");
  const wireframe = new WireframeRenderer(engine);
  const canvas = engine.createView(addCanvas(), "terrain frame smoke");
  canvas.resize(VIEWPORT);
  try {
    field.blocks.forEach((block, n) => {
      if (n === 0) {
        pool.fieldInit(field.body, block);
      } else {
        pool.fieldChunk(field.body, block);
      }
    });
    const held = await until(
      () => pool.fieldBlocks(field.body)?.reported === field.blocks.length,
      FIELD_TIMEOUT_MS,
    );
    if (!held) {
      throw new Error("the workers did not take the fixture's blocks in time");
    }
    const pose = poseOver();
    const pixels = VIEWPORT.widthPx * VIEWPORT.heightPx;
    const bodies = PHOTOREAL_PASS_LABELS.bodies;

    // With relief: the reference surface, then the terrain over it, as the bodies pass draws them.
    const relief = await makePass(null);
    const withRelief = await settled(relief, pool, mask, pose);
    const both = await drawn(engine, pose, [
      {
        label: bodies,
        draws: [...renderer.meshDraws(withRelief.frame.plan), ...withRelief.frame.terrainDraws],
      },
    ]);
    const covered = bodyPixels(both, pixels);
    const finite = both.every(Number.isFinite);
    checks.check(
      "R10.T16.c a photorealistic frame with the body handed over draws its terrain over the reference surface, finite, every pixel drawn",
      withRelief.done && finite && covered === pixels,
      `settled ${String(withRelief.done)}; finite ${String(finite)}, ${String(pixels - covered)} pixels undrawn of ${String(pixels)}; digest ${bytesDigest(both)}`,
    );

    // The same frame by the way back (`GEOMETRY_BEFORE_R14`), from the same selection and residents,
    // as the pass drew it before plan R14, and on its path again after it: byte for byte (R14.T1.a).
    const patches = drawnPatches.get(relief) ?? { interior: 0, edge: 0 };
    const reliefFrame = async (): Promise<Float32Array> => {
      const frame = liveFrame(relief, mask, pose, performance.now());
      return drawn(engine, pose, [
        { label: bodies, draws: [...renderer.meshDraws(frame.plan), ...frame.terrainDraws] },
      ]);
    };
    relief.setGeometry(GEOMETRY_BEFORE_R14);
    const unlevered = await reliefFrame();
    relief.setGeometry(SETTINGS.low.geometry);
    const levered = await reliefFrame();
    const digests = [both, unlevered, levered].map((texels) => bytesDigest(texels));
    if (captures) {
      // The terrain alone for its mask, and the exposure fixed by the way back's brightest terrain.
      const alone = await drawn(engine, pose, [
        {
          label: bodies,
          draws: liveFrame(relief, mask, pose, performance.now()).terrainDraws,
        },
      ]);
      const exposure = captureScale(unlevered, alone);
      images.push(
        captureOf("r14-t1a-frame-way-back", unlevered, VIEWPORT, exposure),
        captureOf("r14-t1a-frame-cluster-path", levered, VIEWPORT, exposure),
        maskOf("r14-t1a-terrain-mask", alone, VIEWPORT),
      );
    }
    const litTriangles =
      (patches.interior * LIT_INTERIOR_INDICES + patches.edge * LIT_PATCH_INDICES) / 3;
    const unleveredTriangles = ((patches.interior + patches.edge) * LIT_PATCH_INDICES) / 3;
    checks.check(
      "R14.T1.a the interior patches' draw, without fins, skirt heads or discard, draws the frame the lit mesh draws, byte for byte",
      withRelief.done && patches.interior > 0 && new Set(digests).size === 1,
      `${String(patches.interior)} interior and ${String(patches.edge)} edge patches: ${String(litTriangles)} lit triangles, ${(litTriangles / unleveredTriangles).toFixed(4)} of the lit mesh's ${String(unleveredTriangles)}; digests ${digests[0] ?? ""} on the cluster path, ${digests[1] ?? ""} by the way back, ${digests[2] ?? ""} on the path again`,
    );

    // R14.T2.a: the lit pass's interior patches through the cull's ordered camera pass, the
    // body's horizon and the stable compaction, against T1.a's interior mesh, from the same
    // selection and residents, in one run; its counters on, so that its triangles are counted.
    relief.measureClusters(true);
    relief.setGeometry({ ...SETTINGS.low.geometry, litClusters: false });
    const byMesh = await reliefFrame();
    // The edge patches by their lit mesh, as T2.a drew them (R14.T1.g is checked apart below).
    relief.setGeometry({ ...SETTINGS.low.geometry, finClusters: false });
    const byClusters = await reliefFrame();
    let litClusterTriangles: number | null = null;
    for (let round = 0; round < 8 && litClusterTriangles === null; round += 1) {
      // Each frame samples the counters the last one's read answers into: one after the other.
      // oxlint-disable-next-line no-await-in-loop
      await reliefFrame();
      litClusterTriangles = relief.counts(VIEW)?.litClusterTriangles ?? null;
    }
    relief.measureClusters(false);
    const ties = differingTexels(byMesh, byClusters);
    if (captures) {
      const exposure = captureScale(byMesh, byMesh);
      images.push(
        captureOf("r14-t2a-frame-interior-mesh", byMesh, VIEWPORT, exposure),
        captureOf("r14-t2a-frame-lit-clusters", byClusters, VIEWPORT, exposure),
      );
    }
    checks.check(
      "R14.T2.a the lit pass's interior patches drawn through the cull and the stable compaction draw T1.a's frame, byte for byte, with fewer lit triangles",
      withRelief.done &&
        ties.pixels === 0 &&
        litClusterTriangles !== null &&
        litClusterTriangles < litTriangles,
      `${String(patches.interior)} interior and ${String(patches.edge)} edge patches: lit triangles ${String(litClusterTriangles)} by clusters, ${litClusterTriangles === null ? "—" : (litClusterTriangles / litTriangles).toFixed(4)} of T1.a's ${String(litTriangles)}; digests ${bytesDigest(byMesh)} by the interior mesh, ${bytesDigest(byClusters)} by clusters; tie pixels ${String(ties.pixels)}, the largest difference ${ties.largest.toExponential(3)}`,
    );

    // R14.T1.g: the edge patches through the cull and the stable compaction beside the interior
    // patches, their fins and skirt heads as `terrain-fin` clusters and every cluster the survey's
    // edge comes near by the fin forms, against their lit mesh's draw, from the same selection and
    // residents, in one run; the counters on, so that both frames' lit triangles are counted.
    const countedFrame = async (
      geometry: GeometrySettings,
    ): Promise<{
      readonly texels: Float32Array;
      readonly counts: TerrainGeometryCounts | null;
      readonly stats: ReadonlyArray<ClusterPassStats> | null;
    }> => {
      relief.setGeometry(geometry);
      const texels = await reliefFrame();
      for (let round = 0; round < 8; round += 1) {
        // Each frame samples the counters some frames' reads answer into: the last of eight.
        // oxlint-disable-next-line no-await-in-loop
        await reliefFrame();
      }
      return { texels, counts: relief.counts(VIEW), stats: relief.clusterStats(VIEW) };
    };
    relief.measureClusters(true);
    const byLitMesh = await countedFrame({ ...SETTINGS.low.geometry, finClusters: false });
    const byFins = await countedFrame(SETTINGS.low.geometry);
    relief.measureClusters(false);
    const finTies = differingTexels(byLitMesh.texels, byFins.texels);
    const finPatches = byFins.counts?.patches ?? NO_DRAWN_PATCHES;
    const meshPatches = byLitMesh.counts?.patches ?? NO_DRAWN_PATCHES;
    const beforeTriangles = byLitMesh.counts?.litClusterTriangles ?? null;
    const afterTriangles = byFins.counts?.litClusterTriangles ?? null;
    // The edge patches' own: the frame's less the interior patches' clusters, the same in both.
    const edgeBefore = (meshPatches.edge * LIT_PATCH_INDICES) / 3;
    const edgeAfter =
      beforeTriangles === null || afterTriangles === null
        ? null
        : afterTriangles - (beforeTriangles - edgeBefore);
    if (captures) {
      const exposure = captureScale(byLitMesh.texels, byLitMesh.texels);
      images.push(
        captureOf("r14-t1g-frame-lit-mesh", byLitMesh.texels, VIEWPORT, exposure),
        captureOf("r14-t1g-frame-fin-clusters", byFins.texels, VIEWPORT, exposure),
      );
    }
    checks.check(
      "R14.T1.g the edge patches drawn through the cull, their fins and skirt heads as clusters and the survey's edge by the fin forms, draw the lit mesh's frame, byte for byte, with fewer lit triangles",
      withRelief.done &&
        finPatches.fins > 0 &&
        meshPatches.fins === 0 &&
        finTies.pixels === 0 &&
        edgeAfter !== null &&
        edgeAfter < edgeBefore,
      `${String(finPatches.interior)} interior and ${String(finPatches.edge)} edge patches, ${String(finPatches.fins)} of them on the cluster path; lit triangles ${String(beforeTriangles)} with the edge patches by the lit mesh, ${String(afterTriangles)} by clusters; the edge patches' ${String(edgeBefore)} by the lit mesh, ${String(edgeAfter)} by clusters, ${edgeAfter === null ? "—" : (edgeAfter / edgeBefore).toFixed(4)} of them; digests ${bytesDigest(byLitMesh.texels)} by the lit mesh, ${bytesDigest(byFins.texels)} by clusters; tie pixels ${String(finTies.pixels)}, the largest difference ${finTies.largest.toExponential(3)}`,
    );

    // R14.T3.c: the lit pass's camera strides, every cluster at the coarsest stride its level's
    // stated error and measured e_s allow at the view's τ′, each vertex they drop morphed onto the
    // coarser mesh, against stride 1 (T1.g's frame by clusters, just drawn), from the same
    // selection and residents, in one run; the counters on, so that its lit triangles are counted.
    // The frames differ where the strides draw coarser ground; the terrain alone, drawn both ways,
    // shows no crack.
    const STRIDED: GeometrySettings = { ...SETTINGS.low.geometry, strides: "measured" };
    const atStride1 = byFins;
    relief.measureClusters(true);
    const strided = await countedFrame(STRIDED);
    relief.measureClusters(false);
    const terrainAloneBy = async (geometry: GeometrySettings): Promise<Float32Array> => {
      relief.setGeometry(geometry);
      return drawn(engine, pose, [
        {
          label: bodies,
          draws: liveFrame(relief, mask, pose, performance.now()).terrainDraws,
        },
      ]);
    };
    const aloneAt1 = await terrainAloneBy(SETTINGS.low.geometry);
    const aloneStrided = await terrainAloneBy(STRIDED);
    relief.setGeometry(SETTINGS.low.geometry);
    const cracks = cracksOf(aloneAt1, aloneStrided);
    const strideChanges = differingTexels(atStride1.texels, strided.texels);
    const strideStops = stopsApart(atStride1.texels, strided.texels, aloneAt1);
    const trianglesAt1 = atStride1.counts?.litClusterTriangles ?? null;
    const trianglesStrided = strided.counts?.litClusterTriangles ?? null;
    if (captures) {
      const exposure = captureScale(atStride1.texels, aloneAt1);
      images.push(
        captureOf("r14-t3c-frame-stride-1", atStride1.texels, VIEWPORT, exposure),
        captureOf("r14-t3c-frame-strides", strided.texels, VIEWPORT, exposure),
        maskOf("r14-t3c-terrain-mask", aloneAt1, VIEWPORT),
      );
    }
    checks.check(
      "R14.T3.c the lit pass's camera strides draw the frame with fewer lit triangles and no crack, their changed pixels listed",
      withRelief.done &&
        cracks === 0 &&
        trianglesAt1 !== null &&
        trianglesStrided !== null &&
        trianglesStrided < trianglesAt1,
      `lit triangles ${String(trianglesAt1)} at stride 1, ${String(trianglesStrided)} with strides, ${trianglesAt1 === null || trianglesStrided === null ? "—" : (trianglesStrided / trianglesAt1).toFixed(4)} of them; ${String(strideChanges.pixels)} pixels changed of ${String(pixels)}, the largest difference ${strideChanges.largest.toExponential(3)}; of ${String(strideStops.terrain)} terrain pixels ${String(strideStops.differing)} differ, ${String(strideStops.within)} within 1/3 stop, the largest ${strideStops.largestStops.toFixed(3)} stops; cracks ${String(cracks)}; digests ${bytesDigest(atStride1.texels)} at stride 1, ${bytesDigest(strided.texels)} with strides`,
    );

    // R14.T3.d: the two-pass occlusion, the lit pass's clusters visible last frame drawn first, the
    // depth pyramid built from that raster, and the rest the pyramid does not refuse drawn over it,
    // against the one raster (T1.g's frame by clusters), from the same selection and residents, in
    // one run: from a frame whose visibility is dropped, whose first raster draws no terrain, and
    // from the next, which draws first what the one before saw; the counters on, so that each pass's
    // clusters are counted, and those the pyramid refused with them.
    const OCCLUDED: GeometrySettings = { ...SETTINGS.low.geometry, occlusion: "hzb" };
    relief.measureClusters(true);
    const oneRaster = await countedFrame(SETTINGS.low.geometry);
    relief.setGeometry(OCCLUDED);
    const pyramidKernel = await engine.createComputeAsync(DEPTH_PYRAMID_KERNEL);
    const pyramid = new DepthPyramid(engine, pyramidKernel, "terrain frame smoke");
    const occludedFrame = async (cut: boolean): Promise<Float32Array> => {
      const fresh = pyramid.begin(VIEWPORT, cut);
      const frame = liveFrame(relief, mask, pose, performance.now(), [], { pyramid, fresh });
      return drawnOccluded(
        engine,
        pose,
        [...renderer.meshDraws(frame.plan), ...frame.terrainDraws],
        pyramid,
        () => frame.terrain.lateDraws(),
      );
    };
    let occludedStats: ReadonlyArray<ClusterPassStats> | null = null;
    let occludedCounts: TerrainGeometryCounts | null = null;
    let firstOccluded: Float32Array;
    let nextOccluded: Float32Array;
    try {
      firstOccluded = await occludedFrame(true);
      nextOccluded = await occludedFrame(false);
      for (let round = 0; round < 8; round += 1) {
        // oxlint-disable-next-line no-await-in-loop
        await occludedFrame(false);
      }
      occludedStats = relief.clusterStats(VIEW);
      occludedCounts = relief.counts(VIEW);
    } finally {
      pyramid.dispose();
      engine.releaseCompute(pyramidKernel);
      relief.measureClusters(false);
      relief.setGeometry(SETTINGS.low.geometry);
    }
    const firstTies = differingTexels(oneRaster.texels, firstOccluded);
    const nextTies = differingTexels(oneRaster.texels, nextOccluded);
    const oneRasterClusters = litOf(oneRaster.stats, CLUSTER_LIT_PASS);
    const earlyClusters = litOf(occludedStats, CLUSTER_LIT_PASS);
    const lateClusters = litOf(occludedStats, CLUSTER_LIT_LATE_PASS);
    const refusedClusters =
      oneRasterClusters === null || earlyClusters === null || lateClusters === null
        ? null
        : oneRasterClusters - earlyClusters - lateClusters;
    if (captures) {
      const exposure = captureScale(oneRaster.texels, oneRaster.texels);
      images.push(
        captureOf("r14-t3d-frame-one-raster", oneRaster.texels, VIEWPORT, exposure),
        captureOf("r14-t3d-frame-occlusion-dropped", firstOccluded, VIEWPORT, exposure),
        captureOf("r14-t3d-frame-occlusion", nextOccluded, VIEWPORT, exposure),
      );
    }
    checks.check(
      "R14.T3.d the two-pass occlusion draws the one raster's frame byte for byte, from a frame whose visibility is dropped and from the next, its refused clusters counted",
      withRelief.done &&
        firstTies.pixels === 0 &&
        nextTies.pixels === 0 &&
        refusedClusters !== null &&
        refusedClusters >= 0 &&
        earlyClusters !== null &&
        earlyClusters > 0,
      `the lit pass's clusters ${String(oneRasterClusters)} in one raster; with the occlusion ${String(earlyClusters)} early and ${String(lateClusters)} late, ${String(refusedClusters)} refused by the pyramid; lit triangles ${String(oneRaster.counts?.litClusterTriangles ?? null)} and ${String(occludedCounts?.litClusterTriangles ?? null)}; tie pixels ${String(firstTies.pixels)} from the dropped frame and ${String(nextTies.pixels)} from the next, the largest difference ${Math.max(firstTies.largest, nextTies.largest).toExponential(3)}; digests ${bytesDigest(oneRaster.texels)} in one raster, ${bytesDigest(firstOccluded)} and ${bytesDigest(nextOccluded)} with the occlusion`,
    );

    // The fallback (plan R14, Design notes 16 and 17), under `--fixture=broken-wgsl` alone: a pass
    // whose cull kernel its compiler refuses draws today's path, the frame the way back draws, and
    // states the refusal.
    if (fixture === "broken-wgsl") {
      const refused = await makePass(null, BROKEN_CULL_KERNEL);
      const settledRefused = await settled(refused, pool, mask, pose);
      const frameBy = async (geometry: GeometrySettings): Promise<Float32Array> => {
        refused.setGeometry(geometry);
        const frame = liveFrame(refused, mask, pose, performance.now());
        return drawn(engine, pose, [
          { label: bodies, draws: [...renderer.meshDraws(frame.plan), ...frame.terrainDraws] },
        ]);
      };
      const asked = bytesDigest(await frameBy(SETTINGS.low.geometry));
      const back = bytesDigest(await frameBy(GEOMETRY_BEFORE_R14));
      const refusal = refused.clusterRefusal ?? "none";
      checks.check(
        "R14.T1.f a refused cull kernel draws today's path, the way back's frame byte for byte, and states its refusal",
        settledRefused.done && asked === back && refusal.startsWith(BROKEN_CULL_KERNEL.name),
        `the cluster path asked ${asked}, the way back ${back}, the first pass's way back ${digests[1] ?? ""}; ${refusal}`,
      );
    }

    // On the datum: each surface alone, through the same live path.
    const datum = await makePass(onTheDatum);
    const onDatum = await settled(datum, pool, mask, pose);
    const terrainAlone = await drawn(engine, pose, [
      { label: "terrain", draws: onDatum.frame.terrainDraws },
    ]);
    const referenceAlone = await drawn(engine, pose, [
      { label: bodies, draws: renderer.meshDraws(onDatum.frame.plan) },
    ]);
    let doubles = 0;
    let terrainPixels = 0;
    let referencePixels = 0;
    for (let p = 0; p < pixels; p += 1) {
      const t = bodyAt(terrainAlone, p);
      const r = bodyAt(referenceAlone, p);
      terrainPixels += t ? 1 : 0;
      referencePixels += r ? 1 : 0;
      doubles += t && r ? 1 : 0;
    }
    checks.check(
      "R10.T16.c on the datum no pixel of the frame is drawn by both the terrain and the reference surface",
      onDatum.done &&
        doubles === 0 &&
        terrainPixels >= 0.1 * pixels &&
        referencePixels >= 0.1 * pixels &&
        terrainAlone.every(Number.isFinite),
      `settled ${String(onDatum.done)}; terrain ${String(terrainPixels)}, reference ${String(referencePixels)}, both ${String(doubles)} of ${String(pixels)}; digests ${bytesDigest(terrainAlone)} terrain, ${bytesDigest(referenceAlone)} reference`,
    );

    // The whole frame through the renderer, the marks under and over the surveyed ground.
    await photoreal.prepare(VIEWPORT, "low", "eye", pose.camera);
    // Each mark along the survey's rows over the surveyed column, apart from the other. The one
    // under the ground stands 15 km beneath the datum: the terrain's depth there is pushed away by
    // the slope term, `occluderSlopePx` pixels of its slope, as the occluders' is: about 1.4 km
    // along the ray at the marks (1.38 to 1.62 km) at this view's pixels (1.8° at its centre, 1.4°
    // the mean), 1.2 to 10 km down its centre column, more where the ground slopes away, and a
    // few tens of metres in a steep view at 1080p. A mark 2 km under the datum stands only 1.5 to
    // 1.7 km under the fixture's ground here (266 m and 495 m below the datum at the marks'
    // ends), within that push over its cased pixels, and showed through (the batch review's S20).
    const fromCamera = (x: number, y: number, upM: number): Vec3 => {
      const { p, nu } = datumAt(OVER, x, y);
      return sub(add(p, scale(nu, upM)), pose.cameraM);
    };
    const under = [fromCamera(48, 0, -15_000), fromCamera(48, 12, -15_000)] as const;
    const over = [fromCamera(48, 30, 2_000), fromCamera(48, 42, 2_000)] as const;
    const list = {
      ...emptyDrawList(viewStrokesAt(1)),
      lines: [markBatch("under the ground", ...under), markBatch("over the ground", ...over)],
    };
    // The frame drawn with `marks` in its symbology, read at the pixels of `at`, from the camera:
    // with or without the terrain's depth in the symbology's pass, and with or without the datum
    // of the body the plan hands over giving way (R10.T16.f), the control as the overlay was built
    // before it, its datum whole; through `relief` from `pose` unless another pass or view is given.
    const drawFrame = async (
      marks: WireframeDrawList,
      options: {
        readonly withDepth: boolean;
        readonly giveWay: boolean;
        readonly pass?: TerrainPass;
        readonly view?: Pose;
      },
      at: ReadonlyArray<Vec3>,
    ): Promise<{
      readonly handedOver: boolean;
      readonly depthDraws: number;
      readonly datums: number;
      readonly shown: ReadonlyArray<boolean>;
      /** How many of the canvas's pixels are the marks' magenta. */
      readonly marked: number;
      /** How many of each row's pixels are, top first. */
      readonly markedRows: ReadonlyArray<number>;
      /** The canvas's fingerprint ({@link bytesDigest}). */
      readonly digest: string;
    }> => {
      const { withDepth, giveWay } = options;
      const view = options.view ?? pose;
      // The marks' camera: only its orientation and field reach their segments, given from it.
      const camera = {
        pose: {
          frame: { kind: "body" as const, body: bodyOf(view).id },
          positionM: view.cameraM,
          orientation: view.camera.orientation,
        },
        fovXRad: view.camera.fovXRad,
      };
      const frame = liveFrame(options.pass ?? relief, mask, view, performance.now());
      const terrain = withDepth
        ? frame.terrain
        : {
            surveyCoverage: (body: string) => frame.terrain.surveyCoverage(body),
            surveyedWhole: (body: string) => frame.terrain.surveyedWhole(body),
            draws: (...args: Parameters<LiveFrameTerrain["draws"]>) => frame.terrain.draws(...args),
            symbologyDepth: () => [],
            sceneDepth: () => false,
            lateDraws: () => [],
          };
      // The datums the renderer packs the overlay with, from its plan.
      let datums = 0;
      const overlay: PhotorealOverlay = (surveyed) => {
        datums = surveyed.size;
        return giveWay
          ? overlaySubmission(wireframe, marks, TOKENS, camera, VIEWPORT, surveyed)
          : overlaySubmission(wireframe, marks, TOKENS, camera, VIEWPORT, new Map());
      };
      const plan = photoreal.render(canvas, {
        camera: view.camera,
        viewport: VIEWPORT,
        presentedPx: VIEWPORT,
        role: "eye",
        setting: "low",
        exposureScale: EXPOSURE,
        sky: [],
        starSprites: [],
        hostDraws: new Map(),
        glareSources: [],
        lights: [
          { disc: sunLikeHostDisc(), centreM: add(frame.body.centreM, scale(view.sun, AU_M)) },
        ],
        bodies: [frame.body],
        depthWriters: [],
        previousRegimes: new Map([[frame.body.id, "terrain"]]),
        terrain,
        overlay,
        meter: null,
      });
      // Read in the task that drew it (`checkPhotorealFrame`'s note).
      const read = await canvas.readBack();
      const rows = markedRows(read);
      return {
        handedOver: plan?.regimes.get(frame.body.id) === "terrain",
        depthDraws: terrain.symbologyDepth().length,
        datums,
        shown: at.map((point) => markedNear(read, project(point, view.camera, VIEWPORT))),
        marked: rows.reduce((sum, count) => sum + count, 0),
        markedRows: rows,
        digest: bytesDigest(read),
      };
    };
    // R14.T1.e: a frame's symbology under each of the overlay's terrain depths, from the same
    // selection and residents: its canvas's fingerprint by the redraw, the cluster path and the
    // scene's depth, then by the redraw again, so that a frame still streaming between them, whose
    // two redraws differ, is told apart.
    const modes: ReadonlyArray<GeometrySettings["overlayDepth"]> = [
      "redraw",
      "cluster",
      "scene-depth",
      "redraw",
    ];
    const overlays: Array<readonly [string, ReadonlyArray<string>]> = [];
    const underEachOverlay = async (
      name: string,
      pass: TerrainPass,
      draw: () => ReturnType<typeof drawFrame>,
    ): Promise<void> => {
      const prints: string[] = [];
      try {
        for (const overlayDepth of modes) {
          pass.setGeometry({ ...SETTINGS.low.geometry, overlayDepth });
          // Each mode's frame is read back before the next is drawn.
          // oxlint-disable-next-line no-await-in-loop
          prints.push((await draw()).digest);
        }
      } finally {
        pass.setGeometry(SETTINGS.low.geometry);
      }
      overlays.push([name, prints]);
    };
    const marksAt = [midpoint(under), midpoint(over)];
    const withDepth = await drawFrame(list, { withDepth: true, giveWay: true }, marksAt);
    await underEachOverlay("the marks under and over the ground", relief, () =>
      drawFrame(list, { withDepth: true, giveWay: true }, marksAt),
    );
    const without = await drawFrame(list, { withDepth: false, giveWay: true }, marksAt);
    const [underShown, overShown] = withDepth.shown;
    const [underBare, overBare] = without.shown;
    checks.check(
      "R10.T16.c over the image the terrain's depth hides a mark under the surveyed ground, and one over it stands",
      withDepth.handedOver &&
        withDepth.depthDraws >= 1 &&
        underShown === false &&
        overShown === true &&
        underBare === true &&
        overBare === true,
      `handed over ${String(withDepth.handedOver)}; with the terrain's ${String(withDepth.depthDraws)} depth draws: the mark under the ground shown ${String(underShown)}, the mark over it ${String(overShown)}; with none: under ${String(underBare)}, over ${String(overBare)}; digests ${withDepth.digest} with depth, ${without.digest} without`,
    );

    // R10.T16.f: two lines of the body's own on the datum, as its graticule is, each along the
    // survey's rows: one over the surveyed column (i 47.75), whose ground stands a few hundred
    // metres under the datum about there, so that its depth leaves the line in sight, and one
    // beyond the survey's +i edge (i 48.25), over the reference surface; with the body's occluder
    // sphere, of its polar radius as the draw list makes it, whose centre places the lines' reads
    // of the mask.
    const datumFromCamera = (key: PatchKey, x: number, y: number): Vec3 =>
      sub(datumAt(key, x, y).p, pose.cameraM);
    const surveyedLine = [datumFromCamera(OVER, 48, 15), datumFromCamera(OVER, 48, 27)] as const;
    const beyondLine = [datumFromCamera(BEYOND, 16, 15), datumFromCamera(BEYOND, 16, 27)] as const;
    const moon = bodyOf(pose);
    const distanceM = norm(moon.centreM);
    const sphereM = occluderRadius(MOON_LIKE.polarRadiusM, distanceM);
    const datumList: WireframeDrawList = {
      ...emptyDrawList(viewStrokesAt(1)),
      occluderSpheres: [
        {
          id: moon.id,
          centreF32: Float32Array.from([moon.centreM.x, moon.centreM.y, moon.centreM.z]),
          radiusM: sphereM,
          altitudeM: distanceM - sphereM,
        },
      ],
      lines: [
        { ...markBatch(`body:${moon.id}:surveyed`, ...surveyedLine), body: moon.id },
        { ...markBatch(`body:${moon.id}:beyond`, ...beyondLine), body: moon.id },
      ],
    };
    const linesAt = [midpoint(surveyedLine), midpoint(beyondLine)];
    const givingWay = await drawFrame(datumList, { withDepth: true, giveWay: true }, linesAt);
    await underEachOverlay("the datum's lines", relief, () =>
      drawFrame(datumList, { withDepth: true, giveWay: true }, linesAt),
    );
    const whole = await drawFrame(datumList, { withDepth: true, giveWay: false }, linesAt);
    const [surveyedShown, beyondShown] = givingWay.shown;
    const [surveyedWhole, beyondWhole] = whole.shown;
    const pixelOf = (point: Vec3): string => {
      const p = project(point, pose.camera, VIEWPORT);
      return `(${p.xPx.toFixed(1)}, ${p.yPx.toFixed(1)})`;
    };
    checks.check(
      "R10.T16.f over the image the datum's line gives way over surveyed ground, and stands beyond the survey's edge",
      givingWay.handedOver &&
        givingWay.datums === 1 &&
        surveyedShown === false &&
        beyondShown === true &&
        surveyedWhole === true &&
        beyondWhole === true,
      `handed over ${String(givingWay.handedOver)}, ${String(givingWay.datums)} datum giving way: the lines over the surveyed ground and beyond its edge, at ${linesAt.map(pixelOf).join(" and ")} px, shown ${String(surveyedShown)} and ${String(beyondShown)}; with no datum giving way, ${String(surveyedWhole)} and ${String(beyondWhole)}; digests ${givingWay.digest} giving way, ${whole.digest} whole`,
    );

    // R14.T3.c: no mark's visibility changes with the camera's strides, the overlay's depth its own
    // pass at stride 1: the marks under and over the ground, and the datum's lines, as at stride 1.
    relief.setGeometry({ ...SETTINGS.low.geometry, strides: "measured" });
    const stridedMarks = await drawFrame(list, { withDepth: true, giveWay: true }, marksAt);
    const stridedLines = await drawFrame(datumList, { withDepth: true, giveWay: true }, linesAt);
    relief.setGeometry(SETTINGS.low.geometry);
    checks.check(
      "R14.T3.c over the image every mark is shown or hidden with the camera's strides as at stride 1",
      sameShown(stridedMarks.shown, withDepth.shown) &&
        sameShown(stridedLines.shown, givingWay.shown),
      `the marks under and over the ground shown ${stridedMarks.shown.join(", ")} with strides, ${withDepth.shown.join(", ")} at stride 1, ${String(stridedMarks.marked)} and ${String(withDepth.marked)} marked pixels; the datum's lines ${stridedLines.shown.join(", ")} and ${givingWay.shown.join(", ")}, ${String(stridedLines.marked)} and ${String(givingWay.marked)} marked pixels`,
    );

    // R10.T16.c's grazing fix, over the flat ground of a pass wholly on the datum, from 10 km up
    // grazing 2° (136 km out over the body's curve), where a row of this view's 5° across 64 px
    // spans some 5.3 km of ground. A mark 1 km behind the ground along the rays through row 23's
    // pixel centres: there the ground those rays meet is the ground the mark stands behind, and the
    // slope term's 3 px pushed it some 16 km behind itself and showed the mark; pushed by 2^-16 of
    // its depth, 2 m, it hides it. (A narrower field streams the finest levels 136 km out, past the
    // part's watchdog.) (The stroke's upper half, over the farther ground of row 22, stands
    // in front of it, as it should.)
    const flat = await makePass(whollyOnTheDatum);
    const graze = grazingPose((2 * Math.PI) / 180, 10_000, (5 * Math.PI) / 180);
    const behind = [20, 44].map((x) => {
      const ground = datumOnRay(graze, x + 0.5, GRAZE_ROW + 0.5);
      return add(ground, scale(normalise(ground), 1_000));
    });
    const [behindStart, behindEnd] = behind;
    if (behindStart === undefined || behindEnd === undefined) {
      throw new Error("the mark has two ends");
    }
    const behindList = {
      ...emptyDrawList(viewStrokesAt(1)),
      lines: [markBatch("behind the ground", behindStart, behindEnd)],
    };
    const grazed = await settled(
      flat,
      pool,
      mask,
      graze,
      [20, 44].map((x) => add(graze.cameraM, datumOnRay(graze, x + 0.5, GRAZE_ROW + 0.5))),
    );
    await photoreal.prepare(VIEWPORT, "low", "eye", graze.camera);
    const behindDrawn = await drawFrame(
      behindList,
      { withDepth: true, giveWay: true, pass: flat, view: graze },
      [],
    );
    const behindBare = await drawFrame(
      behindList,
      { withDepth: false, giveWay: true, pass: flat, view: graze },
      [],
    );
    await underEachOverlay("the mark behind grazing ground", flat, () =>
      drawFrame(behindList, { withDepth: true, giveWay: true, pass: flat, view: graze }, []),
    );
    const markRow = (marked: ReadonlyArray<number>): number => marked[GRAZE_ROW] ?? 0;
    checks.check(
      "R10.T16.c over grazing ground the terrain's depth hides a mark 1 km behind it, seen from 10 km up at 2°",
      grazed.done &&
        behindDrawn.handedOver &&
        behindDrawn.depthDraws >= 1 &&
        markRow(behindDrawn.markedRows) === 0 &&
        markRow(behindBare.markedRows) >= 20,
      `settled ${String(grazed.done)} (${grazed.state}), handed over ${String(behindDrawn.handedOver)}, ${String(behindDrawn.depthDraws)} depth draws: the mark's pixels on its ground's row ${String(markRow(behindDrawn.markedRows))}; with no terrain depth, ${String(markRow(behindBare.markedRows))}; digests ${behindDrawn.digest} with depth, ${behindBare.digest} without`,
    );

    // A line on the ground across the view along row 23's pixel centres, as the survey edge is
    // drawn (its batch's `groundCentreF32`), from 2 km up at 2°, 10° and 60°: drawn whole, every
    // pixel it draws with no terrain depth, where the same line off the ground loses its nearer
    // half to the ground. Its 6 px are wider than the edge's cased 1 px: each fragment of its
    // nearer half is pulled to the datum's plane at its own pixel.
    /** The line's look at `deg`: whether it holds, and what it read. */
    const lineAt = async (
      deg: number,
    ): Promise<{ readonly ok: boolean; readonly read: string }> => {
      const view = grazingPose((deg * Math.PI) / 180, 2_000, (2 * Math.PI) / 180);
      // Three points, so that the chord stands within millimetres of the datum.
      const points = [8, 32, 56].map((x) => datumOnRay(view, x + 0.5, GRAZE_ROW + 0.5));
      const [start, middle, end] = points;
      if (start === undefined || middle === undefined || end === undefined) {
        throw new Error("the line has three points");
      }
      // 6 px wide and uncased, so that its nearer half's rows are counted, wholly covered.
      const line: LineBatch = {
        ...markBatch("on the ground", start, middle),
        segments: Float32Array.from([start, middle, middle, end].flatMap((v) => [v.x, v.y, v.z])),
        widthPx: 6,
        casingWidthPx: 0,
      };
      const onGround: LineBatch = {
        ...line,
        groundCentreF32: Float32Array.from([-view.cameraM.x, -view.cameraM.y, -view.cameraM.z]),
      };
      const viewSettled = await settled(
        flat,
        pool,
        mask,
        view,
        points.map((p) => add(view.cameraM, p)),
      );
      await photoreal.prepare(VIEWPORT, "low", "eye", view.camera);
      const frameOf = (batch: LineBatch, depth: boolean): ReturnType<typeof drawFrame> =>
        drawFrame(
          { ...emptyDrawList(viewStrokesAt(1)), lines: [batch] },
          { withDepth: depth, giveWay: true, pass: flat, view },
          [],
        );
      const grounded = await frameOf(onGround, true);
      await underEachOverlay(`the line on the ground at ${String(deg)}°`, flat, () =>
        frameOf(onGround, true),
      );
      const bare = await frameOf(onGround, false);
      const offGround = await frameOf(line, true);
      return {
        ok:
          viewSettled.done &&
          grounded.depthDraws >= 1 &&
          bare.marked >= 40 &&
          grounded.marked >= 0.98 * bare.marked &&
          offGround.marked <= 0.8 * bare.marked,
        read: `${String(deg)}°: settled ${String(viewSettled.done)} (${viewSettled.state}), on the ground ${String(grounded.marked)} of ${String(bare.marked)} px, off it ${String(offGround.marked)}; digests ${grounded.digest}, ${bare.digest}, ${offGround.digest}`,
      };
    };
    const looks: { readonly ok: boolean; readonly read: string }[] = [];
    for (const deg of [2, 10, 60]) {
      // Each look streams the one pass to its view and draws on the one canvas, so in turn.
      // oxlint-disable-next-line no-await-in-loop
      looks.push(await lineAt(deg));
    }
    checks.check(
      "R10.T16.c a line on the ground, as the survey edge is drawn, draws whole over grazing ground at 2°, 10° and 60°, where one off it loses its nearer half",
      looks.every(({ ok }) => ok),
      looks.map(({ read }) => read).join("; "),
    );

    // R10.T16.c's per-fragment pull: an edge on the ground behind a nearer ridge draws no pixel.
    // From 10 km up at 2°, over a pass wholly on the datum but for a ridge across the view
    // (`ridged`), its crest 11.7 km nearer than a line on the datum along row 23's pixel centres,
    // drawn as the survey edge is at the views' stroke scale of 2: on the ground, 2 px with a 2 px
    // casing. The rays through its marked pixels meet the ridge's face some 13 km in front of it,
    // about 2.5 px of the datum's depth slope there (5.3 km): beyond the 1.7 px they are pulled by,
    // a pixel's and their own offset's, and within the 3 px every fragment was pulled by before
    // (this list's `occluderSlopePx`, at a stroke scale of 1; 5 in the views).
    const ridgeEdge = [8, 32, 56].map((x) => datumOnRay(graze, x + 0.5, GRAZE_ROW + 0.5));
    const [edgeStart, edgeMiddle, edgeEnd] = ridgeEdge;
    if (edgeStart === undefined || edgeMiddle === undefined || edgeEnd === undefined) {
      throw new Error("the edge has three points");
    }
    const ridgePass = await makePass(ridged(graze, add(graze.cameraM, edgeMiddle)));
    const ridgeLine: LineBatch = {
      ...markBatch("behind the ridge", edgeStart, edgeMiddle),
      segments: Float32Array.from(
        [edgeStart, edgeMiddle, edgeMiddle, edgeEnd].flatMap((v) => [v.x, v.y, v.z]),
      ),
      widthPx: 2,
      casingWidthPx: 2,
      groundCentreF32: Float32Array.from([-graze.cameraM.x, -graze.cameraM.y, -graze.cameraM.z]),
    };
    // The edge's points and the ridge's crest on the rays to them, body-fixed.
    const ridgeNear = ridgeEdge.flatMap((p) => [
      add(graze.cameraM, p),
      add(graze.cameraM, scale(p, 1 - RIDGE_LEAD_M / norm(p))),
    ]);
    const ridgeSettled = await settled(ridgePass, pool, mask, graze, ridgeNear);
    await photoreal.prepare(VIEWPORT, "low", "eye", graze.camera);
    const ridgeList = { ...emptyDrawList(viewStrokesAt(1)), lines: [ridgeLine] };
    const behindRidge = await drawFrame(
      ridgeList,
      { withDepth: true, giveWay: true, pass: ridgePass, view: graze },
      [],
    );
    const ridgeBare = await drawFrame(
      ridgeList,
      { withDepth: false, giveWay: true, pass: ridgePass, view: graze },
      [],
    );
    checks.check(
      "R10.T16.c an edge on the ground behind a nearer ridge, by less than the 3 px of slope it was once pulled by, draws no pixel, seen from 10 km up at 2°",
      ridgeSettled.done &&
        behindRidge.handedOver &&
        behindRidge.depthDraws >= 1 &&
        behindRidge.marked === 0 &&
        ridgeBare.marked >= 40,
      `settled ${String(ridgeSettled.done)} (${ridgeSettled.state}), handed over ${String(behindRidge.handedOver)}, ${String(behindRidge.depthDraws)} depth draws: the edge's pixels ${String(behindRidge.marked)} (by row ${behindRidge.markedRows.slice(GRAZE_ROW - 3, GRAZE_ROW + 4).join(" ")} about its row); with no terrain depth, ${String(ridgeBare.marked)}; digests ${behindRidge.digest} with depth, ${ridgeBare.digest} without`,
    );
    // R14.T3.d over the ridge, through the renderer, as `VIEW` draws: the ground behind the ridge
    // is hidden, so the pyramid refuses clusters, and the frame is still the one raster's. The
    // ridge's settle holds its lanes at its first resting frame, whose ground is the survey edge's
    // stand-ins, drawn by the lit mesh alone; this one streams it on, until nothing stands in, to
    // the interior patches the cluster path draws.
    const ridgeStreamed = await settled(ridgePass, pool, mask, graze, null, [], RIDGE_STREAM_MS);
    const overRidge = occlusionSummary(
      await occlusionRun(
        { engine, photoreal, canvas, pass: ridgePass, mask, view: graze },
        SETTINGS.low.geometry,
      ),
    );
    checks.check(
      "R14.T3.d over a ridge the renderer's two-pass occlusion refuses the clusters the ridge hides and draws the one raster's frame, its colour and depth byte for byte, where it drops the visibility and where it keeps it",
      ridgeSettled.done && overRidge.same && overRidge.refused !== null && overRidge.refused > 0,
      `streamed on: ${ridgeStreamed.state}; ${overRidge.text}`,
    );
    // The cluster path is the setting's (plan R14, R14.T1.e's choice); the scene's depth is recorded
    // where it differs, its reference surfaces' depth hiding marks on their datum.
    const steady = overlays.filter(([, [first, , , again]]) => first === again);
    const sceneDiffers = steady.filter(([, [first, , scene]]) => scene !== first);
    checks.check(
      "R14.T1.e every mark over the image is kept or hidden as the redraw keeps or hides it: on the cluster path the symbology draws the redraw's canvas, byte for byte, wherever the frame held still",
      overlays.length === 6 &&
        steady.length >= 2 &&
        steady.every(([, [first, cluster]]) => cluster === first),
      `${String(steady.length)} of ${String(overlays.length)} frames held still; the scene's depth differs at ${sceneDiffers.map(([name]) => name).join(", ") || "none"}; ${overlays
        .map(([name, prints]) => `${name}: ${prints.join(", ")}`)
        .join("; ")} (redraw, cluster, scene depth, redraw)`,
    );
  } finally {
    canvas.dispose();
    wireframe.dispose();
    photoreal.dispose();
    renderer.dispose();
    for (const pass of passes) {
      pass.dispose();
    }
    offChange();
    pool.terminate();
    mask.release();
  }
  return images;
}

/**
 * R14.T3.c's grounded craft (Design note 10; `R14.T3.c the ground under a grounded craft`), in a
 * part of its own: the frame check's world, workers and coverage mask made again, and one lit
 * pass on the low setting, its changes cut at their first draw, as the frame check's are.
 */
export async function checkStridesUnderCraft(engine: RenderEngine, checks: Checks): Promise<void> {
  const field = moonLikeFixture("first");
  const pool = new HeightWorkerPool({
    workers: 2,
    createWorker: () =>
      new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
        type: "module",
      }),
  });
  const survey = new SurveyedCells();
  const mask = new CoverageMask(engine, "strides under a craft smoke: coverage mask");
  const offChange = pool.onFieldChange((change) => {
    survey.add(change);
    mask.survey(change);
  });
  const passes: TerrainPass[] = [];
  const makePass = async (): Promise<TerrainPass> => {
    const hold = new LaneHold();
    const pass = new TerrainPass({
      engine,
      setting: "low",
      terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
      geometry: smokeGeometry("low"),
      planet: planetGeometry(MOON_LIKE_FIGURE, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      createPool: (bake) => hold.wrap(pool.lane(bake)),
      survey,
    });
    pass.setReducedMotion(true);
    laneHolds.set(pass, hold);
    passes.push(pass);
    await pass.ready();
    return pass;
  };
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  try {
    field.blocks.forEach((block, n) => {
      if (n === 0) {
        pool.fieldInit(field.body, block);
      } else {
        pool.fieldChunk(field.body, block);
      }
    });
    const held = await until(
      () => pool.fieldBlocks(field.body)?.reported === field.blocks.length,
      FIELD_TIMEOUT_MS,
    );
    if (!held) {
      throw new Error("the workers did not take the fixture's blocks in time");
    }
    const bodies = PHOTOREAL_PASS_LABELS.bodies;
    const STRIDED: GeometrySettings = { ...SETTINGS.low.geometry, strides: "measured" };
    // R14.T3.c (Design note 10): a craft of 20 m grounded on the surveyed column, seen from 1.5 km
    // over it at 70° below the horizon through a field of 8°, every patch it sees its own bake.
    // Its ground with the camera's strides is the stride-1 ground, byte for byte, at every pixel
    // whose ground lies within half its held radius: the forced region at stride 1, and its
    // vertices' stride morph held at 0 there as the CDLOD morph is. Elsewhere the strides move
    // pixels.
    const foot = datumAt(OVER, 48, 20);
    const groundPass = await makePass();
    await settled(groundPass, pool, mask, grazingPose(RAD_70, 1_500, RAD_8, 48, 20), [foot.p]);
    const groundUnder = heldGroundUnder(MOON_LIKE_FIGURE, groundPass.heights);
    const craft: GroundContact = { positionM: groundUnder(foot.p).groundPointM, radiusM: 20 };
    // From 1.5 km over the craft's ground, looking at it, through a field of 8°.
    const overhead = poseLookingAt(craft.positionM, foot.nu, RAD_70, 1_500, RAD_8);
    const groundSettled = await settled(
      groundPass,
      pool,
      mask,
      overhead,
      null,
      [craft],
      CRAFT_TIMEOUT_MS,
    );
    const groundFrameBy = async (geometry: GeometrySettings): Promise<Float32Array> => {
      groundPass.setGeometry(geometry);
      const frame = liveFrame(groundPass, mask, overhead, performance.now(), [craft]);
      return drawn(engine, overhead, [
        { label: bodies, draws: [...renderer.meshDraws(frame.plan), ...frame.terrainDraws] },
      ]);
    };
    const groundAt1 = await groundFrameBy(SETTINGS.low.geometry);
    const groundStrided = await groundFrameBy(STRIDED);
    const heldM = heldRadiusM(craft, finestPatchSizeM(groundPass.planet));
    const craftRadiusM = norm(craft.positionM);
    let underCraft = 0;
    let underDiffering = 0;
    for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
      for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
        const ground = add(overhead.cameraM, datumOnRay(overhead, x + 0.5, y + 0.5, craftRadiusM));
        if (norm(sub(ground, craft.positionM)) > 0.5 * heldM) {
          continue;
        }
        underCraft += 1;
        const p = 4 * (y * VIEWPORT.widthPx + x);
        for (let c = 0; c < 4; c += 1) {
          if (groundAt1[p + c] !== groundStrided[p + c]) {
            underDiffering += 1;
            break;
          }
        }
      }
    }
    const groundChanges = differingTexels(groundAt1, groundStrided);
    checks.check(
      "R14.T3.c under a grounded craft the ground with the camera's strides is the stride-1 ground, byte for byte (Design note 10)",
      groundSettled.done && underCraft >= 16 && underDiffering === 0 && groundChanges.pixels > 0,
      `settled ${String(groundSettled.done)} (${groundSettled.state}); held radius ${heldM.toFixed(1)} m: ${String(underCraft)} pixels under half of it, ${String(underDiffering)} of them differing; ${String(groundChanges.pixels)} pixels of the frame changed; digests ${bytesDigest(groundAt1)} at stride 1, ${bytesDigest(groundStrided)} with strides`,
    );
  } finally {
    renderer.dispose();
    for (const pass of passes) {
      pass.dispose();
    }
    offChange();
    pool.terminate();
    mask.release();
  }
}
