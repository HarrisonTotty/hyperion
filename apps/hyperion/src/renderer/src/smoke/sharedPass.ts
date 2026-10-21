/**
 * The smoke harness's shared-pass check (plan R10, R10.T16.b, `R10.T16 the shared pass`): one lit
 * terrain pass over the Moon-like fixture's survey, shared by two views, on the real device, as
 * `VIEW`'s terrain session runs it.
 *
 * @remarks
 * The world is `hyperion_surface`'s Moon-like synthetic world, both its payloads' blocks
 * (`tests/golden/worker/moon_like.golden`), taken by a pool of two height workers made without
 * bake settings, as the session's is; each pass bakes through a lane of it, its selection reading
 * the survey the blocks brought. On an engine of its own, so that its forced device loss leaves the
 * page's other groups alone, it checks:
 *
 * - three passes made, drawn and disposed leave the engine's allocation tally, its bytes by
 *   category, where it stood before the first (decision-r10-t16-contracts §1);
 * - two views of one pass, from two poses over the survey, each draw a finite frame holding the
 *   terrain at its pose;
 * - after the device is destroyed while the pass streams, the terrain returns, baked through a new
 *   lane of the same pool: no worker is made again and no block is posted again.
 *
 * A buffer or texture beyond the device's limits, or a write outside one, raises an uncaptured
 * validation error, which the harness's main process counts against the run.
 *
 * Beside it, on the page's own engine, whose variant decides the capability path, the high
 * setting's shadow cascades (plan R10, R10.T9.b; decision-r10-t9b-design D2): one high-setting
 * pass with cascades of its own streams a view 40 m over the fixture's ground under the centre
 * patch, looking west 20° below the horizon, the sun 12° up in the east, and draws one frame with
 * its body taking the cascades. It checks that the four caster passes are drawn at w > 0, the
 * nearest with the terrain's draw, on the path the device's `depth-clip-control` gives (logged:
 * unclipped, or pulled back); that `shadows` holds 67,108,864 B while the set lives and its figure
 * from before once it is let go; and that the frame is finite at every texel, darker than the same
 * draw at w = 0 at some (the cascades shadow something the horizon map does not: 585 of 2,108
 * terrain texels on SwiftShader with the cascades fitted to the disc's centre, before
 * decision-r10-t9b-penumbra's fit to its top edge, which leaves only where the two disagree) and
 * nowhere brighter, since the minimum never brightens.
 */

import { withTemplateCurve } from "../view/appearance/phase";
import type { Quaternion } from "../view/camera/pose";
import { viewRotation4 } from "../view/camera/projection";
import { quaternionFromRows } from "../view/camera/quaternion";
import { viewId } from "../view/camera/state";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import type { MemoryCategory } from "../view/engine/memory";
import type { DrawItem, RenderEngine } from "../view/engine/types";
import { annulusEdges } from "../view/lighting/annuli";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { vertexDir } from "../view/terrain/cube";
import { allocationTally } from "../view/terrain/gpu/allocationTally";
import type { PatchKey } from "../view/terrain/patchKey";
import { planetGeometry, spheroidNormal, surfacePoint } from "../view/terrain/planet";
import { CASCADE_BYTES, CASCADE_MEMORY_CATEGORY } from "../view/terrain/shadows/cascades";
import { TerrainShadows } from "../view/terrain/shadows/terrainShadows";
import { SurveyedCells } from "../view/terrain/surveyLookup";
import type { TerrainLighting } from "../view/terrain/terrainMaterial";
import {
  TerrainPass,
  type TerrainPassView,
  type TerrainView,
  type TerrainViewFrame,
} from "../view/terrain/terrainPass";
import { HeightWorkerPool } from "../view/terrain/workers/pool";
import { moonLikeFixture } from "../test/moonLikeFixture";
import { type Checks, halfTexels, pause, projection } from "./harness";
import { MOON_LIKE } from "./litTerrain";

/** Each view's frame, pixels. */
const FRAME = { widthPx: 64, heightPx: 48 } as const;

/** The horizontal field of view `projection` draws at: 2 atan(aspect), its x scale 1 ÷ aspect. */
const FOV_X_RAD = 2 * Math.atan(FRAME.widthPx / FRAME.heightPx);

/** The passes' cache budget, 32 MiB: a few hundred lit slots, as the lit-terrain check takes. */
const CACHE_BYTES = 2 ** 25;

/** How long the workers may take over the field, and a pass over its streaming, milliseconds. */
const FIELD_TIMEOUT_MS = 60_000;
const STREAM_TIMEOUT_MS = 90_000;

/** How long the engine may take to rebuild after its device is destroyed, milliseconds. */
const RESTORE_TIMEOUT_MS = 10_000;

/** Every memory category, for the tally's bytes. */
const CATEGORIES: ReadonlyArray<MemoryCategory> = [
  "render-targets",
  "other",
  "atmosphere-tables",
  "atmosphere-view",
  "coarse-field-gpu",
  "height-cache",
  "shadows",
  "sky-cube",
  "sky-scratch",
];

/** The patch the views look at: level 7 over the first survey's cells of face +y. */
const CENTRE: PatchKey = { face: 1, level: 7, i: 45, j: 45 };

interface Xyz {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

function vector([x, y, z]: readonly [number, number, number]): Xyz {
  return { x, y, z };
}

function cross(a: Xyz, b: Xyz): Xyz {
  return { x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x };
}

function along(a: Xyz, ka: number, b: Xyz, kb: number): Xyz {
  return { x: a.x * ka + b.x * kb, y: a.y * ka + b.y * kb, z: a.z * ka + b.z * kb };
}

function unit(v: Xyz): Xyz {
  const n = Math.hypot(v.x, v.y, v.z);
  return { x: v.x / n, y: v.y / n, z: v.z / n };
}

/** The datum's normal and its east, over the centre patch. */
function frameOverCentre(): { readonly nu: Xyz; readonly east: Xyz; readonly north: Xyz } {
  const dir = vertexDir(CENTRE, 32, 32);
  const nu = vector(spheroidNormal(MOON_LIKE_FIGURE, dir));
  const reference = Math.abs(nu.z) > 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 0, z: 1 };
  const east = unit(cross(reference, nu));
  return { nu, east, north: cross(nu, east) };
}

/** The Moon-like figure, with no pole known: selection reads none. */
const MOON_LIKE_FIGURE = { ...MOON_LIKE, pole: null };

/**
 * A view `heightM` above the centre patch, looking `depressionDeg` below the horizon towards a
 * heading `headingDeg` east of north; the body does not turn, so its axes are the view's.
 */
function viewOver(heightM: number, depressionDeg: number, headingDeg: number): TerrainView {
  const dir = vertexDir(CENTRE, 32, 32);
  return viewFrom(vector(surfacePoint(MOON_LIKE_FIGURE, dir, heightM)), depressionDeg, headingDeg);
}

/**
 * A view from `camera`, body-fixed, looking `depressionDeg` below the centre patch's horizon
 * towards a heading `headingDeg` east of north.
 */
function viewFrom(camera: Xyz, depressionDeg: number, headingDeg: number): TerrainView {
  const { nu, east, north } = frameOverCentre();
  const h = (headingDeg * Math.PI) / 180;
  const ahead = along(north, Math.cos(h), east, Math.sin(h));
  const d = (depressionDeg * Math.PI) / 180;
  const forward = along(ahead, Math.cos(d), nu, -Math.sin(d));
  const up = along(ahead, Math.sin(d), nu, Math.cos(d));
  const right = cross(forward, up);
  const orientation: Quaternion = quaternionFromRows([
    { x: right.x, y: up.x, z: -forward.x },
    { x: right.y, y: up.y, z: -forward.y },
    { x: right.z, y: up.z, z: -forward.z },
  ]);
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: camera,
    orientation,
    fovXRad: FOV_X_RAD,
    viewport: FRAME,
  };
}

/**
 * The views' lights: a Sun-like star `elevationRad` above the centre patch's horizon towards the
 * east, 40° from the zenith by default, at 1 au (IAU 2012 B2's 1.495978707 × 10¹¹ m) with the
 * Sun's nominal radius (IAU 2015 B3's 6.957 × 10⁸ m) and about the Sun's illuminance at 1 au,
 * relative to the centre ÷ a, body-fixed, as the lit-terrain check's.
 */
function lighting(elevationRad = Math.PI / 2 - 0.7): TerrainLighting {
  const a = MOON_LIKE.equatorialRadiusM;
  const { nu, east } = frameOverCentre();
  const annuli = annulusEdges(0.6, 0.2, 2);
  return {
    lights: [
      {
        direction: unit(along(nu, Math.sin(elevationRad), east, Math.cos(elevationRad))),
        distance: 1.495978707e11 / a,
        radius: 6.957e8 / a,
        illuminance: [1.3e5, 1.25e5, 1.2e5],
        annuli: [annuli, annuli, annuli],
      },
    ],
    occluders: [],
    secondaries: [],
    elsewhere: withTemplateCurve({
      a: [0.11, 0.11, 0.11],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    }),
    exposureScale: 1e-4,
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

/** One view's frame drawn and read back. */
interface Drawn {
  readonly finite: boolean;
  /** The share of the frame the terrain covers. */
  readonly cover: number;
}

/** Draws `frame`'s draw from `view` into a frame of its own and reads it back. */
async function drawView(
  engine: RenderEngine,
  name: string,
  view: TerrainView,
  frame: TerrainViewFrame,
): Promise<Drawn> {
  const target = engine.createRenderTarget({
    name,
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  try {
    target.render({
      label: name,
      viewRotation: viewRotation4(view.orientation),
      projection: projection(FRAME.widthPx / FRAME.heightPx),
      draws: frame.draw === null ? [] : [frame.draw],
      postProcesses: [],
    });
    const texels = halfTexels(await engine.readTexture(target.colour));
    let terrain = 0;
    for (let p = 0; p < FRAME.widthPx * FRAME.heightPx; p += 1) {
      // The meter class: 2 lit by a star, 3 unlit (R07's `METER_CLASS`).
      const meter = texels[4 * p + 3];
      if (meter === 2 || meter === 3) {
        terrain += 1;
      }
    }
    return {
      finite: texels.every(Number.isFinite),
      cover: terrain / (FRAME.widthPx * FRAME.heightPx),
    };
  } finally {
    target.dispose();
  }
}

/**
 * Steps `pass` over `views` and draws each, every 50 ms, until every view's draw set leaves nothing
 * missing or standing in, or the time runs out; the last frames, by view.
 */
async function stream(
  pass: TerrainPass,
  views: ReadonlyArray<TerrainPassView>,
  timeoutMs: number,
): Promise<{ readonly settled: boolean; readonly frames: ReadonlyArray<TerrainViewFrame> }> {
  const started = performance.now();
  const light = lighting();
  for (;;) {
    const now = performance.now();
    pass.step({ views, grounded: [] });
    const frames = views.map(({ id, view }) =>
      pass.drawFor(id, {
        view,
        sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
        sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
        exposureScale: light.exposureScale,
        lighting: light,
        nowMs: now,
      }),
    );
    const settled = frames.every((f) => !f.missing && !f.standingIn && f.draw !== null);
    if (settled || now - started > timeoutMs) {
      return { settled, frames };
    }
    // Streaming runs in the workers; give them the event loop between frames.
    // oxlint-disable-next-line no-await-in-loop
    await pause(50);
  }
}

/** R10.T16's shared pass: see the module's remarks. */
export async function checkSharedPass(
  engine: RenderEngine,
  device: GPUDevice | undefined,
  checks: Checks,
): Promise<void> {
  const field = moonLikeFixture("all");
  const tally = allocationTally(engine);
  let workersMade = 0;
  const pool = new HeightWorkerPool({
    workers: 2,
    createWorker: () => {
      workersMade += 1;
      return new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
        type: "module",
      });
    },
  });
  const survey = new SurveyedCells();
  const offChange = pool.onFieldChange((change) => {
    survey.add(change);
  });
  const passes: TerrainPass[] = [];
  const makePass = (): TerrainPass => {
    const pass = new TerrainPass({
      engine,
      setting: "low",
      terrain: { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES },
      planet: planetGeometry(MOON_LIKE_FIGURE, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      createPool: (bake) => pool.lane(bake),
      survey,
    });
    passes.push(pass);
    return pass;
  };
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
    const primary: TerrainPassView = {
      id: viewId("view"),
      view: viewOver(30_000, 60, 0),
      tauPx: TERRAIN_SETTINGS.low.tauPx,
      weight: 1,
    };
    const instrument: TerrainPassView = {
      id: viewId("instrument 1"),
      view: viewOver(60_000, 75, 90),
      tauPx: TERRAIN_SETTINGS.low.tauPx,
      weight: 0.25,
    };

    // Three passes made, drawn and disposed: the tally where it stood before the first.
    const before = CATEGORIES.map((c) => tally.liveBytes(c));
    for (let n = 0; n < 3; n += 1) {
      const pass = makePass();
      // oxlint-disable-next-line no-await-in-loop
      await pass.ready();
      // oxlint-disable-next-line no-await-in-loop
      await stream(pass, [primary], 2_000);
      pass.dispose();
    }
    const after = CATEGORIES.map((c) => tally.liveBytes(c));
    // The passes did hold their caches and frame buffers while they lived.
    const heldCache = tally.peakBytes("height-cache");
    const heldOther = tally.peakBytes("other");
    checks.check(
      "R10.T16.b a pass made, drawn and disposed three times leaves the allocation tally where it stood",
      after.every((bytes, n) => bytes === before[n]) && heldCache > 0 && heldOther > 0,
      `${CATEGORIES.map((c, n) => `${c} ${String(before[n])} → ${String(after[n])} B`).join(", ")}; peaks while they lived: height-cache ${String(heldCache)} B, other ${String(heldOther)} B`,
    );

    // Two views of one pass.
    const pass = makePass();
    await pass.ready();
    const twoViews = await stream(pass, [primary, instrument], STREAM_TIMEOUT_MS);
    const drawn = [
      await drawView(engine, "shared pass: primary", primary.view, twoViews.frames[0] ?? fail()),
      await drawView(
        engine,
        "shared pass: instrument",
        instrument.view,
        twoViews.frames[1] ?? fail(),
      ),
    ];
    checks.check(
      "R10.T16.b two views of one lit pass each draw a finite frame of the terrain at its pose",
      twoViews.settled && drawn.every((d) => d.finite && d.cover >= 0.3),
      `settled ${String(twoViews.settled)}; ${drawn
        .map(
          (d, n) =>
            `view ${String(n)}: terrain ${(100 * d.cover).toFixed(1)}%, finite ${String(d.finite)}`,
        )
        .join("; ")}`,
    );

    // The device destroyed while the pass streams a new pose: the terrain returns, on a new lane.
    if (device === undefined) {
      throw new Error("the shared pass's engine's device was not recorded");
    }
    const closer: TerrainPassView = { ...primary, view: viewOver(8_000, 50, 45) };
    pass.step({ views: [closer], grounded: [] });
    const blocks = pool.fieldBlocks(field.body)?.posted ?? -1;
    const made = workersMade;
    let restored = 0;
    const stop = engine.onRestored(() => {
      restored += 1;
    });
    device.destroy();
    const back = await until(() => restored > 0, RESTORE_TIMEOUT_MS);
    stop();
    // The pass's rebuild compiles its material again; the stream waits for its draw.
    const returned = await stream(pass, [closer], STREAM_TIMEOUT_MS);
    const afterLoss = await drawView(
      engine,
      "shared pass: after loss",
      closer.view,
      returned.frames[0] ?? fail(),
    );
    checks.check(
      "R10.T16.b after a device destroyed mid-stream the terrain returns, with no block posted again",
      back &&
        returned.settled &&
        afterLoss.finite &&
        afterLoss.cover >= 0.3 &&
        workersMade === made &&
        pool.fieldBlocks(field.body)?.posted === blocks &&
        pool.lostWorkers === 0,
      `restored ${String(restored)}, settled ${String(returned.settled)}, terrain ${(100 * afterLoss.cover).toFixed(1)}%, finite ${String(afterLoss.finite)}, workers made ${String(workersMade - made)} more, blocks posted ${String((pool.fieldBlocks(field.body)?.posted ?? 0) - blocks)} more`,
    );
  } finally {
    for (const pass of passes) {
      pass.dispose();
    }
    offChange();
    pool.terminate();
    tally.dispose();
  }
}

/** Fails the check where a frame it needs is missing. */
function fail(): never {
  throw new Error("a view's frame is missing");
}

/** The cascades' camera's height above the fixture's ground under it, m (D2: a few to 100 m). */
const CASCADES_HEIGHT_M = 40;

/** The cascades' view's depression below the horizon and heading, west, away from the sun, °. */
const CASCADES_DEPRESSION_DEG = 20;
const CASCADES_HEADING_DEG = 270;

/** The sun's elevation in the cascades' frame, towards the east, rad: 12°, low, so shadows are long. */
const CASCADES_SUN_RAD = (12 * Math.PI) / 180;

/** How long the cascades' pass may stream before its frame is drawn, ms (part 7's watchdog). */
const CASCADES_STREAM_MS = 25_000;

/**
 * The fixture's realised relief, m: the worker golden's `realised_relief_m`, 14,921.5 m (plan 10,
 * R10.T9.a's figures, S6), for the cascades' caster reach.
 */
const MOON_LIKE_RELIEF_M = 14_921.5;

/** How much brighter than at w = 0 a texel of the frame with cascades may be, relative. */
const NEVER_BRIGHTER = 1e-3;

/** `view`'s frame inputs under `light`, its body taking the cascades or not. */
function frameInput(
  view: TerrainView,
  light: TerrainLighting,
  takesCascades: boolean,
  nowMs: number,
): Parameters<TerrainPass["drawFor"]>[1] {
  return {
    view,
    sunDirectionBodyFixed: light.lights[0]?.direction ?? { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: light.exposureScale,
    lighting: light,
    takesCascades,
    nowMs,
  };
}

/** `draws` drawn from `view` into a frame of their own, read back four channels a texel. */
async function texelsOf(
  engine: RenderEngine,
  name: string,
  view: TerrainView,
  draws: ReadonlyArray<DrawItem>,
): Promise<Float32Array> {
  const target = engine.createRenderTarget({
    name,
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  try {
    target.render({
      label: name,
      viewRotation: viewRotation4(view.orientation),
      projection: projection(FRAME.widthPx / FRAME.heightPx),
      draws,
      postProcesses: [],
    });
    return halfTexels(await engine.readTexture(target.colour));
  } finally {
    target.dispose();
  }
}

/**
 * The frame with cascades against the same draw at w = 0, over the texels both draw as terrain:
 * how many, how many are darker by more than {@link NEVER_BRIGHTER}, and the most any is brighter,
 * relative, a channel under a millionth of the frame's brightest taken at that millionth.
 */
function againstUnshadowed(
  shadowed: Float32Array,
  open: Float32Array,
): { readonly terrain: number; readonly darker: number; readonly brighter: number } {
  let brightest = 0;
  for (let p = 0; p < FRAME.widthPx * FRAME.heightPx; p += 1) {
    brightest = Math.max(brightest, open[4 * p] ?? 0, open[4 * p + 1] ?? 0, open[4 * p + 2] ?? 0);
  }
  const floor = Math.max(1e-6 * brightest, Number.MIN_VALUE);
  let terrain = 0;
  let darker = 0;
  let brighter = 0;
  for (let p = 0; p < FRAME.widthPx * FRAME.heightPx; p += 1) {
    const meter = open[4 * p + 3];
    if (meter !== 2 && meter !== 3) {
      continue;
    }
    terrain += 1;
    let dark = false;
    for (let c = 0; c < 3; c += 1) {
      const lit = Math.max(open[4 * p + c] ?? Number.NaN, floor);
      const seen = shadowed[4 * p + c] ?? Number.NaN;
      brighter = Math.max(brighter, (seen - lit) / lit);
      dark ||= seen < lit * (1 - NEVER_BRIGHTER);
    }
    darker += dark ? 1 : 0;
  }
  return { terrain, darker, brighter };
}

/** R10.T9.b's cascades on the high setting: see the module's remarks. */
export async function checkShadowCascades(engine: RenderEngine, checks: Checks): Promise<void> {
  const unclipped = engine.capabilities.depthClipControl;
  const path = unclipped
    ? "unclipped: depth-clip-control, the casters pancaked"
    : "pulled back: no depth-clip-control, the near plane at the caster reach";
  const field = moonLikeFixture("first");
  const tally = allocationTally(engine);
  const pool = new HeightWorkerPool({
    workers: 2,
    createWorker: () =>
      new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
        type: "module",
      }),
  });
  const survey = new SurveyedCells();
  const offChange = pool.onFieldChange((change) => {
    survey.add(change);
  });
  const before = tally.liveBytes(CASCADE_MEMORY_CATEGORY);
  const shadows = new TerrainShadows(engine, { cascades: true });
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
    // The field's own ground under the centre patch, along the datum's normal, which the drawn
    // ground converges on as the finer bakes land.
    const { nu } = frameOverCentre();
    const ground = await pool.groundReadout(field.body, field.seed, {
      originM: vector(surfacePoint(MOON_LIKE_FIGURE, vertexDir(CENTRE, 32, 32), 20_000)),
      direction: { x: -nu.x, y: -nu.y, z: -nu.z },
      maxM: 40_000,
      slopeBaselineM: 2,
      padWidthM: 1,
      seasonMeanAnomalyRad: undefined,
    });
    if (ground?.kind !== "ground") {
      throw new Error(`the cascades' pose has no ground under it: ${ground?.kind ?? "no field"}`);
    }
    const view = viewFrom(
      along(vector(ground.sight.hitM), 1, nu, CASCADES_HEIGHT_M),
      CASCADES_DEPRESSION_DEG,
      CASCADES_HEADING_DEG,
    );
    const made = new TerrainPass({
      engine,
      setting: "high",
      terrain: { ...TERRAIN_SETTINGS.high, cacheBytes: CACHE_BYTES },
      planet: planetGeometry(MOON_LIKE_FIGURE, pool.fieldLevels(field.body)),
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      createPool: (bake) => pool.lane(bake),
      survey,
      shadows,
      reliefM: () => MOON_LIKE_RELIEF_M,
    });
    pass = made;
    await made.ready();
    const passView: TerrainPassView = {
      id: viewId("cascades"),
      view,
      tauPx: TERRAIN_SETTINGS.high.tauPx,
      weight: 1,
    };
    const light = lighting(CASCADES_SUN_RAD);
    // Streamed without the cascades, so that SwiftShader rasterises their 2,048² layers once.
    const started = performance.now();
    let settled = false;
    while (!settled && performance.now() - started < CASCADES_STREAM_MS) {
      made.step({ views: [passView], grounded: [] });
      const streaming = made.drawFor(
        passView.id,
        frameInput(view, light, false, performance.now()),
      );
      settled = !streaming.missing && !streaming.standingIn && streaming.draw !== null;
      if (!settled) {
        // The workers bake and the engine answers on the event loop, between the polls.
        // oxlint-disable-next-line no-await-in-loop
        await pause(50);
      }
    }
    made.step({ views: [passView], grounded: [] });
    const frame = made.drawFor(passView.id, frameInput(view, light, true, performance.now()));
    const living = tally.liveBytes(CASCADE_MEMORY_CATEGORY) - before;
    const cascades = frame.cascades;
    const passes = cascades?.passes ?? [];
    const draws = passes.map((each) => each.draws.length);
    const weight = cascades?.set.weight ?? 0;
    checks.check(
      "R10.T9.b on the high setting near the ground the four caster passes are drawn at w > 0, the nearest with the terrain",
      frame.draw !== null &&
        weight > 0 &&
        passes.length === 4 &&
        passes.every((each, n) => each.layer === n) &&
        (draws[0] ?? 0) >= 1 &&
        cascades?.set.unclippedDepth === unclipped,
      `${path}; settled ${String(settled)} in ${((performance.now() - started) / 1_000).toFixed(1)} s; w ${weight.toFixed(3)}, shadowed range ${(cascades?.set.rangeM ?? 0).toFixed(0)} m, cascade 1's texel ${((cascades?.set.cascades[0].texelM ?? 0) * 100).toFixed(2)} cm, caster reach ${((cascades?.set.casterReachM ?? 0) / 1_000).toFixed(1)} km; draws per cascade ${draws.join(", ")}`,
    );
    const draw = frame.draw;
    if (draw === null) {
      throw new Error("the cascades' view drew no terrain");
    }
    // The receiver's read as drawn, then the same draw at w = 0, before any other draws the set.
    const shadowed = await texelsOf(engine, "cascades: shadowed", view, [draw]);
    const axis = draw.uniforms["cascadeViewAxis"] ?? new Float32Array(4);
    const open = await texelsOf(engine, "cascades: w = 0", view, [
      {
        ...draw,
        uniforms: {
          ...draw.uniforms,
          cascadeViewAxis: Float32Array.of(axis[0] ?? 0, axis[1] ?? 0, axis[2] ?? 0, 0),
        },
      },
    ]);
    const finite = shadowed.every(Number.isFinite) && open.every(Number.isFinite);
    const seen = againstUnshadowed(shadowed, open);
    checks.check(
      "R10.T9.b the frame with cascades is finite at every texel, darker at some and nowhere brighter than at w = 0",
      finite &&
        seen.terrain >= 0.3 * FRAME.widthPx * FRAME.heightPx &&
        seen.darker > 0 &&
        seen.brighter <= NEVER_BRIGHTER,
      `${path}; finite ${String(finite)}; terrain ${String(seen.terrain)} texels, ${String(seen.darker)} darker under the cascades, the most brighter ${seen.brighter.toExponential(2)} (at most ${String(NEVER_BRIGHTER)})`,
    );
    made.dispose();
    pass = null;
    shadows.dispose();
    const after = tally.liveBytes(CASCADE_MEMORY_CATEGORY);
    checks.check(
      "R10.T9.b the cascades' 67,108,864 B are counted under shadows while they live, and released after",
      living === CASCADE_BYTES && after === before,
      `${path}; shadows ${String(before)} B before, ${String(before + living)} B while the set lived (${String(living)} B, expected ${String(CASCADE_BYTES)} B), ${String(after)} B after`,
    );
  } finally {
    pass?.dispose();
    shadows.dispose();
    offChange();
    pool.terminate();
    tally.dispose();
  }
}
