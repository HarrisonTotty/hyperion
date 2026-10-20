/**
 * The smoke harness's wireframe-terrain check (plan R10, R10.T11, `R10.T11 the wireframe's
 * terrain`): the Moon-like fixture's surveyed ground drawn in the wireframe style, as depth with its
 * contours and graticule, by a wireframe pass of the terrain session's kind, through R02's renderer
 * onto a canvas, on the real device.
 *
 * @remarks
 * The world is the Moon-like fixture's first survey, taken by a pool of two height workers made
 * without bake settings, as the terrain session's is; its blocks feed the body's survey, which
 * selection reads, and its GPU coverage mask. One wireframe pass (`weights` `"skipped"`) bakes
 * through a lane of the pool at the wireframe's τ, from the survey-edge check's pose, 30 km over the
 * first survey's +i edge about 60° below the horizon (`poseOver`), the surveyed ground on the
 * view's lower rows and the unsurveyed beyond the edge on its upper ones. The colours are the
 * check's own, so that each kind of line is told by its channel: the contours and the graticule's
 * strokes green, the graticule's casing red, the marks magenta. It checks:
 *
 * - the frame streams to a draw set with nothing missing, the view given the body's mask;
 * - its terrain draws contours (green pixels) and the graticule on them (its red casing), and its
 *   depth hides a mark 15 km under the surveyed ground while one 2 km over it stands; drawn with
 *   no terrain, both stand;
 * - the body's datum gives way to its terrain where its mask holds the ground surveyed: a line of
 *   the body's at the datum across the edge is drawn beyond the edge and not over the survey, and
 *   its occluder sphere hides a mark 15 km under the datum beyond the edge but not over the survey;
 *   with no mask given, the line is drawn and the mark hidden on both sides;
 * - the pass made, drawn and disposed leaves the engine's allocation tally, its bytes by category,
 *   where it stood.
 *
 * A buffer or texture beyond the device's limits, or a write outside one, raises an uncaptured
 * validation error, which the harness's main process counts against the run.
 */

import { add, norm, scale, sub, type Vec3 } from "../geometry/vec3";
import { project } from "../view/camera/projection";
import { viewId } from "../view/camera/state";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import { occluderRadius } from "../view/depth/depth";
import type { MemoryCategory } from "../view/engine/memory";
import type { RenderEngine } from "../view/engine/types";
import { rotationColumns } from "../view/bodies/smoothMesh";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { CoverageMask } from "../view/terrain/coverage";
import { allocationTally } from "../view/terrain/gpu/allocationTally";
import { handoverTauPx } from "../view/terrain/handover";
import type { PatchKey } from "../view/terrain/patchKey";
import { planetGeometry } from "../view/terrain/planet";
import { SurveyedCells } from "../view/terrain/surveyLookup";
import {
  TerrainPass,
  type TerrainView,
  type TerrainWireFrameInput,
} from "../view/terrain/terrainPass";
import { HeightWorkerPool } from "../view/terrain/workers/pool";
import {
  type DrawCamera,
  emptyDrawList,
  type LineBatch,
  type OccluderSphere,
  STROKE_PX,
  viewStrokesAt,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import {
  linearColour,
  NO_WIREFRAME_TERRAIN,
  type WireframeTerrain,
  WireframeRenderer,
} from "../view/wireframe/submit";
import { CASING_PX } from "../lib/strokes";
import { moonLikeFixture } from "../test/moonLikeFixture";
import { addCanvas } from "./frames";
import { type Checks, pause } from "./harness";
import { MOON_LIKE } from "./litTerrain";
import { datumAt, OVER, type Pose, poseOver, VIEWPORT } from "./surveyEdge";

/** The pass's cache budget, 32 MiB, as the other terrain checks take. */
const CACHE_BYTES = 2 ** 25;

/** How long the workers may take over the field, and the pass over its streaming, milliseconds. */
const FIELD_TIMEOUT_MS = 60_000;
const STREAM_TIMEOUT_MS = 90_000;

/** The view's name, as the session knows `VIEW`'s primary. */
const VIEW = viewId("view");

/** The Moon-like figure with no pole known, as the pass's selection reads it. */
const MOON_LIKE_FIGURE = { ...MOON_LIKE, pole: null };

/** The fixture's body. */
const BODY = "5e5e000000000000.0003";

/**
 * The check's contour interval, m: 1 km, whose lines stand some ten pixels apart over the
 * fixture's relief from this pose, inside the fade at the view's stroke scale of 2.
 */
const INTERVAL_M = 1_000;

/**
 * The check's graticule step, rad: a quarter of a degree, 7.6 km on the Moon-like body, so that its
 * lines cross the view (R02's own steps, 15° and 30°, would leave this small a view empty).
 */
const GRATICULE_STEP_RAD = (0.25 * Math.PI) / 180;

/** The check's colours, told by their channels: green lines, a red casing, magenta marks. */
const LINE = "#00ff00";
const CASING = "#ff0000";
const MARK = "#ff00ff";

/** The patch beyond the survey's +i edge, across from {@link OVER}. */
const BEYOND: PatchKey = { ...OVER, i: OVER.i + 1 };

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

/** The pass's view of the body from `pose`: body-fixed axes the frame's. */
function viewOf(pose: Pose): TerrainView {
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: pose.cameraM,
    orientation: pose.camera.orientation,
    fovXRad: pose.camera.fovXRad,
    viewport: VIEWPORT,
  };
}

/** The frame's wireframe input: the check's interval, graticule and colours at a stroke scale of 2. */
function wireOf(occluderSlopePx: number, strokeScale: number): TerrainWireFrameInput {
  return {
    intervalM: INTERVAL_M,
    seaLevelM: null,
    strokeScale,
    graticule: {
      stepRad: GRATICULE_STEP_RAD,
      thinPx: STROKE_PX.thin * strokeScale,
      heavyPx: STROKE_PX.heavy * strokeScale,
      casingPx: CASING_PX * strokeScale,
    },
    colours: { muted: linearColour(LINE), text: linearColour(LINE), casing: linearColour(CASING) },
    occluderSlopePx,
  };
}

/** A view-space line from `a` to `b`, m from the camera, in `colour`, of the body's where given. */
function lineBatch(id: string, a: Vec3, b: Vec3, colour: string, body?: string): LineBatch {
  return {
    id,
    space: "view",
    originF32: new Float32Array(3),
    segments: Float32Array.from([a.x, a.y, a.z, b.x, b.y, b.z]),
    token: "text",
    colour,
    widthPx: 2,
    casingWidthPx: 0,
    casingColour: "#000000",
    dash: null,
    ...(body === undefined ? {} : { body }),
  };
}

/** A pixel's channels from a canvas read-back, 0 to 1, in RGB order whatever the canvas's. */
type Pixel = readonly [number, number, number];

/** Reads pixel `n` of `read`, red, green and blue, in either channel order the canvas has. */
function pixelAt(read: Float32Array | Uint8Array, n: number, bgra: boolean): Pixel {
  const full = read instanceof Uint8Array ? 255 : 1;
  const c0 = (read[4 * n] ?? 0) / full;
  const c1 = (read[4 * n + 1] ?? 0) / full;
  const c2 = (read[4 * n + 2] ?? 0) / full;
  return bgra ? [c2, c1, c0] : [c0, c1, c2];
}

/** Whether a pixel is the marks' magenta, red and blue near full and green near none. */
function isMark(p: Pixel): boolean {
  return p[0] > 0.8 && p[1] < 0.3 && p[2] > 0.8;
}

/** Whether a pixel is a line's green, or a red casing's, alone. */
function isGreen(p: Pixel): boolean {
  return p[1] > 0.3 && p[0] < 0.3 && p[2] < 0.3;
}

function isRed(p: Pixel): boolean {
  return p[0] > 0.3 && p[1] < 0.3 && p[2] < 0.3;
}

/** Whether any pixel within one of `at` satisfies `test`. */
function near(
  read: Float32Array | Uint8Array,
  at: { readonly xPx: number; readonly yPx: number },
  test: (p: Pixel) => boolean,
  bgra: boolean,
): boolean {
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      const x = Math.floor(at.xPx) + dx;
      const y = Math.floor(at.yPx) + dy;
      if (x >= 0 && y >= 0 && x < VIEWPORT.widthPx && y < VIEWPORT.heightPx) {
        if (test(pixelAt(read, y * VIEWPORT.widthPx + x, bgra))) {
          return true;
        }
      }
    }
  }
  return false;
}

/** R10.T11's wireframe terrain: see the module's remarks. */
export async function checkWireframeTerrain(engine: RenderEngine, checks: Checks): Promise<void> {
  const field = moonLikeFixture("first");
  const tally = allocationTally(engine);
  const categories: ReadonlyArray<MemoryCategory> = ["height-cache", "other"];
  const pool = new HeightWorkerPool({
    workers: 2,
    createWorker: () =>
      new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
        type: "module",
      }),
  });
  const survey = new SurveyedCells();
  const mask = new CoverageMask(engine, "wireframe terrain smoke: coverage mask");
  const offChange = pool.onFieldChange((change) => {
    survey.add(change);
    mask.survey(change);
  });
  const wireframe = new WireframeRenderer(engine);
  const canvas = engine.createView(addCanvas(), "wireframe terrain smoke");
  canvas.resize(VIEWPORT);
  let pass: TerrainPass | null = null;
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
    const strokes = viewStrokesAt(1);
    const base = emptyDrawList(strokes);
    const wire = wireOf(base.occluderSlopePx, strokes.strokeScale);
    const camera: DrawCamera = {
      pose: {
        frame: { kind: "body", body: BODY },
        positionM: pose.cameraM,
        orientation: pose.camera.orientation,
      },
      fovXRad: pose.camera.fovXRad,
    };
    // A frame drawn first, so that the canvas's own targets are made before the tally is read.
    wireframe.render(canvas, base, camera, VIEWPORT);
    const before = categories.map((category) => tally.liveBytes(category));
    const made = new TerrainPass({
      engine,
      setting: "low",
      terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
      planet: planetGeometry(MOON_LIKE_FIGURE, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "skipped" },
      createPool: (bake) => pool.lane(bake),
      survey,
    });
    pass = made;
    await made.ready();
    // One frame of the pass's view, as `VIEW`'s loop steps it and its drawer draws it.
    const frameAt = (nowMs: number): ReturnType<TerrainPass["drawFor"]> => {
      made.step({
        views: [
          { id: VIEW, view: viewOf(pose), tauPx: handoverTauPx("wireframe", "low"), weight: 1 },
        ],
        grounded: [],
      });
      return made.drawFor(VIEW, {
        view: viewOf(pose),
        sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
        sunIlluminanceLx: [0, 0, 0],
        exposureScale: 1,
        wire,
        nowMs,
      });
    };
    const started = performance.now();
    let frame = frameAt(started);
    for (;;) {
      const drawSet = made.lastStep?.drawSet;
      const done =
        frame.draw !== null &&
        frame.maskReady &&
        drawSet !== undefined &&
        drawSet.missing === 0 &&
        drawSet.standingIn === 0;
      if (done || performance.now() - started > STREAM_TIMEOUT_MS) {
        break;
      }
      // Streaming runs in the workers; give them the event loop between frames.
      // oxlint-disable-next-line no-await-in-loop
      await pause(50);
      frame = frameAt(performance.now());
    }
    const settled = frame.draw !== null && frame.maskReady;
    checks.check(
      "R10.T11 a wireframe pass streams the surveyed ground to a draw with the body's mask given",
      settled,
      `draw ${String(frame.draw !== null)}, mask ready ${String(frame.maskReady)}, missing ${String(frame.missing)}`,
    );
    const texture = mask.texture;
    if (frame.draw === null || texture === null) {
      return;
    }

    const draw = frame.draw;
    const drawWith = async (
      list: WireframeDrawList,
      terrain: WireframeTerrain,
    ): Promise<Float32Array | Uint8Array> => {
      wireframe.render(canvas, list, camera, VIEWPORT, [], terrain);
      return canvas.readBack();
    };
    const surveyed: WireframeTerrain["surveyed"] = new Map([
      [BODY, { mask: texture, axes: rotationColumns(IDENTITY_ROTATION), stretch: 1 }],
    ]);
    const fromCamera = (key: PatchKey, x: number, y: number, upM: number): Vec3 => {
      const { p, nu } = datumAt(key, x, y);
      return sub(add(p, scale(nu, upM)), pose.cameraM);
    };
    // A red probe across the view's middle row, in screen space, by which the canvas's channel
    // order is told.
    const probe = await drawWith(
      {
        ...base,
        lines: [
          {
            ...lineBatch("probe", { x: 8, y: 24, z: 0 }, { x: 56, y: 24, z: 0 }, CASING),
            space: "screen",
          },
        ],
      },
      NO_WIREFRAME_TERRAIN,
    );
    const bgra = !near(probe, { xPx: 32, yPx: 24 }, isRed, false);

    // The terrain with its lines, and marks under and over the surveyed ground.
    const under = [fromCamera(OVER, 48, 0, -15_000), fromCamera(OVER, 48, 12, -15_000)] as const;
    const over = [fromCamera(OVER, 48, 30, 2_000), fromCamera(OVER, 48, 42, 2_000)] as const;
    const marks = {
      ...base,
      lines: [lineBatch("under", ...under, MARK), lineBatch("over", ...over, MARK)],
    };
    const markAt = (pair: readonly [Vec3, Vec3]): { xPx: number; yPx: number } =>
      project(scale(add(pair[0], pair[1]), 0.5), pose.camera, VIEWPORT);
    const withTerrain = await drawWith(marks, { draws: [draw], surveyed });
    const withoutTerrain = await drawWith(marks, NO_WIREFRAME_TERRAIN);
    let green = 0;
    let red = 0;
    for (let n = 0; n < VIEWPORT.widthPx * VIEWPORT.heightPx; n += 1) {
      const p = pixelAt(withTerrain, n, bgra);
      green += isGreen(p) ? 1 : 0;
      red += isRed(p) ? 1 : 0;
    }
    const underShown = near(withTerrain, markAt(under), isMark, bgra);
    const overShown = near(withTerrain, markAt(over), isMark, bgra);
    const underBare = near(withoutTerrain, markAt(under), isMark, bgra);
    const overBare = near(withoutTerrain, markAt(over), isMark, bgra);
    checks.check(
      "R10.T11 the wireframe's terrain draws its contours and graticule, and its depth hides a mark under the surveyed ground while one over it stands",
      green > 0 && red > 0 && !underShown && overShown && underBare && overBare,
      `${String(green)} contour or graticule pixels, ${String(red)} casing pixels; with the terrain the mark under shown ${String(underShown)}, over ${String(overShown)}; with none under ${String(underBare)}, over ${String(overBare)}`,
    );

    // The datum gives way where the mask holds the ground surveyed: the body's own line at the
    // datum across the edge, and a mark under the datum behind its occluder sphere, each on both
    // sides of the edge.
    const datumLine = lineBatch(
      "datum",
      fromCamera(OVER, 40, 24, 0),
      fromCamera(BEYOND, 24, 24, 0),
      LINE,
      BODY,
    );
    const lineOver = project(fromCamera(OVER, 48, 24, 0), pose.camera, VIEWPORT);
    const lineBeyond = project(fromCamera(BEYOND, 16, 24, 0), pose.camera, VIEWPORT);
    const sunken = [
      [fromCamera(OVER, 44, 20, -15_000), fromCamera(OVER, 52, 28, -15_000)],
      // Far enough beyond that the ray to it meets the sphere beyond the edge too: from 30 km up,
      // about 60° down, a point 15 km under the datum lies some 26 quads past where its ray
      // crosses the datum.
      [fromCamera(BEYOND, 48, 20, -15_000), fromCamera(BEYOND, 56, 28, -15_000)],
    ] as const;
    const centreM = scale(pose.cameraM, -1);
    const distanceM = norm(centreM);
    const radiusM = occluderRadius(MOON_LIKE.polarRadiusM, distanceM);
    const sphere: OccluderSphere = {
      id: BODY,
      centreF32: Float32Array.from([centreM.x, centreM.y, centreM.z]),
      radiusM,
      altitudeM: distanceM - radiusM,
    };
    const datumList = {
      ...base,
      occluderSpheres: [sphere],
      lines: [
        datumLine,
        lineBatch("sunken over", ...sunken[0], MARK),
        lineBatch("sunken beyond", ...sunken[1], MARK),
      ],
    };
    const masked = await drawWith(datumList, { draws: [], surveyed });
    const unmasked = await drawWith(datumList, NO_WIREFRAME_TERRAIN);
    const seen = (read: Float32Array | Uint8Array): readonly boolean[] => [
      near(read, lineOver, isGreen, bgra),
      near(read, lineBeyond, isGreen, bgra),
      near(read, markAt(sunken[0]), isMark, bgra),
      near(read, markAt(sunken[1]), isMark, bgra),
    ];
    const [lineOverMasked, lineBeyondMasked, sunkenOverMasked, sunkenBeyondMasked] = seen(masked);
    const [lineOverBare, lineBeyondBare, sunkenOverBare, sunkenBeyondBare] = seen(unmasked);
    checks.check(
      "R10.T11 the body's datum gives way to its terrain where its mask holds the ground surveyed: its line is not drawn and its occluder hides nothing there",
      lineOverMasked === false &&
        lineBeyondMasked === true &&
        sunkenOverMasked === true &&
        sunkenBeyondMasked === false &&
        lineOverBare === true &&
        lineBeyondBare === true &&
        sunkenOverBare === false &&
        sunkenBeyondBare === false,
      `with the mask: its line over the survey ${String(lineOverMasked)}, beyond ${String(lineBeyondMasked)}; a mark 15 km under the datum over it ${String(sunkenOverMasked)}, beyond ${String(sunkenBeyondMasked)}; with none: line ${String(lineOverBare)}, ${String(lineBeyondBare)}; mark ${String(sunkenOverBare)}, ${String(sunkenBeyondBare)}`,
    );

    made.dispose();
    pass = null;
    const after = categories.map((category) => tally.liveBytes(category));
    checks.check(
      "R10.T11 a wireframe pass made, drawn and disposed leaves the allocation tally where it stood",
      after.every((bytes, n) => bytes === before[n]),
      categories
        .map((category, n) => `${category} ${String(before[n])} B, then ${String(after[n])} B`)
        .join("; "),
    );
  } finally {
    pass?.dispose();
    canvas.dispose();
    wireframe.dispose();
    offChange();
    pool.terminate();
    mask.release();
  }
}
