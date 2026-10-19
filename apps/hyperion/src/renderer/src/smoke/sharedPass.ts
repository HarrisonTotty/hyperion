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
 */

import { withTemplateCurve } from "../view/appearance/phase";
import type { Quaternion } from "../view/camera/pose";
import { viewRotation4 } from "../view/camera/projection";
import { quaternionFromRows } from "../view/camera/quaternion";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import type { MemoryCategory } from "../view/engine/memory";
import type { RenderEngine } from "../view/engine/types";
import { annulusEdges } from "../view/lighting/annuli";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { vertexDir } from "../view/terrain/cube";
import { allocationTally } from "../view/terrain/gpu/allocationTally";
import type { PatchKey } from "../view/terrain/patchKey";
import { planetGeometry, spheroidNormal, surfacePoint } from "../view/terrain/planet";
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
  const camera = vector(surfacePoint(MOON_LIKE_FIGURE, dir, heightM));
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
 * The views' lights: a Sun-like star 40° from the zenith towards the east, at 1 au (IAU 2012 B2's
 * 1.495978707 × 10¹¹ m) with the Sun's nominal radius (IAU 2015 B3's 6.957 × 10⁸ m) and about the
 * Sun's illuminance at 1 au, relative to the centre ÷ a, body-fixed, as the lit-terrain check's.
 */
function lighting(): TerrainLighting {
  const a = MOON_LIKE.equatorialRadiusM;
  const { nu, east } = frameOverCentre();
  const annuli = annulusEdges(0.6, 0.2, 2);
  return {
    lights: [
      {
        direction: unit(along(nu, Math.cos(0.7), east, Math.sin(0.7))),
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
      id: "view",
      view: viewOver(30_000, 60, 0),
      tauPx: TERRAIN_SETTINGS.low.tauPx,
      weight: 1,
    };
    const instrument: TerrainPassView = {
      id: "instrument 1",
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
