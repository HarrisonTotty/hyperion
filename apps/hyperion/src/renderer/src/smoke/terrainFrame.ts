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
 *   overlay was built before R10.T16.f, both stand.
 */

import { add, norm, scale, sub, type Vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import {
  type BodyFrameOptions,
  type BodyFramePlan,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../view/bodies/draw";
import { project } from "../view/camera/projection";
import { viewId } from "../view/camera/state";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import { occluderRadius } from "../view/depth/depth";
import type { RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { overlaySubmission } from "../view/photoreal/overlay";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { type PhotorealOverlay, PhotorealRenderer } from "../view/photoreal/renderer";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import { CoverageMask } from "../view/terrain/coverage";
import {
  type FrameTerrainSource,
  LiveFrameTerrain,
  terrainViewOf,
} from "../view/terrain/frameTerrain";
import { handoverTauPx, limbDistanceM, pixelAngleRad } from "../view/terrain/handover";
import type { PatchKey } from "../view/terrain/patchKey";
import { planetGeometry } from "../view/terrain/planet";
import { SurveyedCells } from "../view/terrain/surveyLookup";
import { TerrainPass, type TerrainPool } from "../view/terrain/terrainPass";
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
import { type Checks, pause } from "./harness";
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

/** The passes' cache budget, 32 MiB: a few hundred lit slots, as the survey-edge check takes. */
const CACHE_BYTES = 2 ** 25;

/** How long the workers may take over the field, and a pass over its streaming, milliseconds. */
const FIELD_TIMEOUT_MS = 60_000;
const STREAM_TIMEOUT_MS = 90_000;

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

/** A lane of the pool whose bakes are laid on the datum before the pass takes them. */
function onTheDatumLane(lane: HeightWorkerLane): TerrainPool {
  return {
    reprioritise: (demand) => {
      lane.reprioritise(demand);
    },
    onBaked: (cb) =>
      lane.onBaked((bake) => {
        cb(onTheDatum(bake));
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

/** The view's frame terrain over `pass` and the body's mask, `VIEW`'s session's seams. */
function sourceOf(pass: TerrainPass, mask: CoverageMask): FrameTerrainSource {
  return {
    drawTerrain: (view, _body, input) => pass.drawFor(view, input),
    coverageMask: () => mask.texture,
    pass: () => pass,
  };
}

/** Steps `pass` from `pose` and makes the frame, as `VIEW`'s loop and drawer do. */
function liveFrame(pass: TerrainPass, mask: CoverageMask, pose: Pose, nowMs: number): LiveFrame {
  const base = bodyOf(pose);
  const view = terrainViewOf(base, pose.camera, VIEWPORT);
  if (view === null) {
    throw new Error("the body turns");
  }
  pass.step({
    views: [{ id: VIEW, view, tauPx: handoverTauPx("photorealistic", "low"), weight: 1 }],
    grounded: [],
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
      : { ...base, surveyCoverage: { mask: coverage, rotation: IDENTITY_ROTATION } };
  const plan = planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(body.centreM, scale(pose.sun, AU_M)) }],
    optionsOf(pose),
    new Map([[body.id, "disc"]]),
  );
  return { plan, body, terrain, terrainDraws: terrain.draws(plan, [body]) };
}

/** Streams `pass` until its view's frame gives the mask and draws its terrain whole, or time runs out. */
async function settled(
  pass: TerrainPass,
  mask: CoverageMask,
  pose: Pose,
): Promise<{ readonly done: boolean; readonly frame: LiveFrame }> {
  const started = performance.now();
  for (;;) {
    const now = performance.now();
    const frame = liveFrame(pass, mask, pose, now);
    const drawSet = pass.lastStep?.drawSet;
    const role = frame.plan.meshes[0]?.role;
    const done =
      role?.kind === "handed-over" &&
      role.coverage !== null &&
      frame.terrainDraws.length === 1 &&
      drawSet !== undefined &&
      drawSet.missing === 0 &&
      drawSet.standingIn === 0;
    if (done || now - started > STREAM_TIMEOUT_MS) {
      return { done, frame };
    }
    // Streaming runs in the workers; give them the event loop between frames.
    // oxlint-disable-next-line no-await-in-loop
    await pause(50);
  }
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
 * Whether any pixel within one of `at` on the canvas read-back is the marks' magenta: red and blue
 * near full and green near none, in either channel order and either encoding the canvas has.
 */
function markedNear(
  read: Float32Array | Uint8Array,
  at: { readonly xPx: number; readonly yPx: number },
): boolean {
  const full = read instanceof Uint8Array ? 255 : 1;
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      const x = Math.floor(at.xPx) + dx;
      const y = Math.floor(at.yPx) + dy;
      if (x < 0 || y < 0 || x >= VIEWPORT.widthPx || y >= VIEWPORT.heightPx) {
        continue;
      }
      const n = 4 * (y * VIEWPORT.widthPx + x);
      const [r = 0, g = full, b = 0] = [read[n], read[n + 1], read[n + 2]];
      if (r > 0.8 * full && g < 0.3 * full && b > 0.8 * full) {
        return true;
      }
    }
  }
  return false;
}

/** R10.T16.c's frame: see the module's remarks. */
export async function checkTerrainFrame(engine: RenderEngine, checks: Checks): Promise<void> {
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
  const makePass = async (onDatum: boolean): Promise<TerrainPass> => {
    const pass = new TerrainPass({
      engine,
      setting: "low",
      terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
      planet: planetGeometry(MOON_LIKE_FIGURE, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      createPool: (bake) => (onDatum ? onTheDatumLane(pool.lane(bake)) : pool.lane(bake)),
      survey,
    });
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
    const relief = await makePass(false);
    const withRelief = await settled(relief, mask, pose);
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
      `settled ${String(withRelief.done)}; finite ${String(finite)}, ${String(pixels - covered)} pixels undrawn of ${String(pixels)}`,
    );

    // On the datum: each surface alone, through the same live path.
    const datum = await makePass(true);
    const onDatum = await settled(datum, mask, pose);
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
      `settled ${String(onDatum.done)}; terrain ${String(terrainPixels)}, reference ${String(referencePixels)}, both ${String(doubles)} of ${String(pixels)}`,
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
    // The marks' camera: only its orientation and field reach their segments, given from it.
    const camera = {
      pose: {
        frame: { kind: "body" as const, body: bodyOf(pose).id },
        positionM: pose.cameraM,
        orientation: pose.camera.orientation,
      },
      fovXRad: pose.camera.fovXRad,
    };
    // The frame drawn with `marks` in its symbology, read at the pixels of `at`, from the camera:
    // with or without the terrain's depth in the symbology's pass, and with or without the datum
    // of the body the plan hands over giving way (R10.T16.f), the control as the overlay was built
    // before it, its datum whole.
    const drawFrame = async (
      marks: WireframeDrawList,
      options: { readonly withDepth: boolean; readonly giveWay: boolean },
      at: ReadonlyArray<Vec3>,
    ): Promise<{
      readonly handedOver: boolean;
      readonly depthDraws: number;
      readonly datums: number;
      readonly shown: ReadonlyArray<boolean>;
    }> => {
      const { withDepth, giveWay } = options;
      const frame = liveFrame(relief, mask, pose, performance.now());
      const terrain = withDepth
        ? frame.terrain
        : {
            surveyCoverage: (body: string) => frame.terrain.surveyCoverage(body),
            draws: (...args: Parameters<LiveFrameTerrain["draws"]>) => frame.terrain.draws(...args),
            symbologyDepth: () => [],
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
        camera: pose.camera,
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
          { disc: sunLikeHostDisc(), centreM: add(frame.body.centreM, scale(pose.sun, AU_M)) },
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
      return {
        handedOver: plan?.regimes.get(frame.body.id) === "terrain",
        depthDraws: terrain.symbologyDepth().length,
        datums,
        shown: at.map((point) => markedNear(read, project(point, pose.camera, VIEWPORT))),
      };
    };
    const marksAt = [midpoint(under), midpoint(over)];
    const withDepth = await drawFrame(list, { withDepth: true, giveWay: true }, marksAt);
    const without = await drawFrame(list, { withDepth: false, giveWay: true }, marksAt);
    const [underShown, overShown] = withDepth.shown;
    const [underBare, overBare] = without.shown;
    checks.check(
      "R10.T16.c over the image the terrain's depth hides a mark under the surveyed ground, and one over it stands",
      withDepth.handedOver &&
        withDepth.depthDraws === 1 &&
        underShown === false &&
        overShown === true &&
        underBare === true &&
        overBare === true,
      `handed over ${String(withDepth.handedOver)}; with the terrain's ${String(withDepth.depthDraws)} depth draw: the mark under the ground shown ${String(underShown)}, the mark over it ${String(overShown)}; with none: under ${String(underBare)}, over ${String(overBare)}`,
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
      `handed over ${String(givingWay.handedOver)}, ${String(givingWay.datums)} datum giving way: the lines over the surveyed ground and beyond its edge, at ${linesAt.map(pixelOf).join(" and ")} px, shown ${String(surveyedShown)} and ${String(beyondShown)}; with no datum giving way, ${String(surveyedWhole)} and ${String(beyondWhole)}`,
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
}
