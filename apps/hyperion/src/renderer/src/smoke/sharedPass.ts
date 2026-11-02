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
 * nowhere brighter, since the minimum never brightens. Then plan R14's levers against the way back
 * (T1–T1.d), and the far cascades at a rate of 4 against every frame over four frames flown at
 * R05's descent's 300 m/s under a setting sun (T6.b): the passes each frame renders, every
 * surface of every frame's far maps held within a texel, the frame within a third of a stop at
 * 99.5% of its terrain texels, and its acne unchanged; its caster triangles by frame logged.
 */

import { withTemplateCurve } from "../view/appearance/phase";
import type { Quaternion } from "../view/camera/pose";
import { viewRotation4 } from "../view/camera/projection";
import { quaternionFromRows } from "../view/camera/quaternion";
import { viewId } from "../view/camera/state";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import type { MemoryCategory } from "../view/engine/memory";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { KernelPair } from "../view/engine/kernels";
import type { DrawItem, RenderEngine, TextureHandle } from "../view/engine/types";
import { annulusEdges } from "../view/lighting/annuli";
import type { ClusterPassStats } from "../view/clusters/stats";
import {
  GEOMETRY_BEFORE_R14,
  type GeometrySettings,
  SETTINGS,
  TERRAIN_SETTINGS,
} from "../view/quality/qualitySetting";
import { LIT_PATCH_INDICES } from "../view/terrain/gpu/resources";
import { vertexDir } from "../view/terrain/cube";
import { allocationTally } from "../view/terrain/gpu/allocationTally";
import type { PatchKey } from "../view/terrain/patchKey";
import { planetGeometry, spheroidNormal, surfacePoint } from "../view/terrain/planet";
import {
  CASCADE_BYTES,
  CASCADE_MEMORY_CATEGORY,
  FAR_CASCADE_SUN_SHARE,
} from "../view/terrain/shadows/cascades";
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
import type { CapturedImage } from "./atmosphere";
import { bytesDigest, type Checks, halfTexels, pause, projection } from "./harness";
import {
  captureOf,
  captureScale,
  isTerrain,
  luminanceAt,
  maskOf,
  stopsApart,
} from "./leverCaptures";
import { MOON_LIKE } from "./litTerrain";
import { smokeGeometry } from "./geometry";

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
  "geometry-clusters",
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
  /** Its texels' fingerprint ({@link bytesDigest}). */
  readonly digest: string;
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
      draws: frame.draws,
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
      digest: bytesDigest(texels),
    };
  } finally {
    target.dispose();
  }
}

/**
 * Steps `pass` over `views` and draws each, every 50 ms, until every view's draw set leaves nothing
 * missing or standing in and no draft waits for its refined normals (R10.T6.a), or the time runs
 * out; the last frames, by view.
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
    const settled =
      frames.every((f) => !f.missing && !f.standingIn && f.draws.length > 0) && pass.refining === 0;
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
      geometry: smokeGeometry("low"),
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

    // Three passes made, drawn and disposed: the tally where it stood before the first, but for the
    // engine's own pass timer, which its first timed pass makes for the engine's life (the cluster
    // path's cull is one, plan R14).
    let timerBytes = 0;
    const offTimer = engine.onAllocation((event) => {
      if (event.kind !== "uploaded" && event.category === "other" && isPassTimer(event.name)) {
        timerBytes += event.kind === "created" ? event.bytes : -event.bytes;
      }
    });
    const before = CATEGORIES.map((c) => tally.liveBytes(c));
    for (let n = 0; n < 3; n += 1) {
      const pass = makePass();
      // oxlint-disable-next-line no-await-in-loop
      await pass.ready();
      // oxlint-disable-next-line no-await-in-loop
      await stream(pass, [primary], 2_000);
      pass.dispose();
    }
    offTimer();
    const after = CATEGORIES.map((c) => tally.liveBytes(c) - (c === "other" ? timerBytes : 0));
    // The passes did hold their caches and frame buffers while they lived.
    const heldCache = tally.peakBytes("height-cache");
    const heldOther = tally.peakBytes("other");
    checks.check(
      "R10.T16.b a pass made, drawn and disposed three times leaves the allocation tally where it stood",
      after.every((bytes, n) => bytes === before[n]) && heldCache > 0 && heldOther > 0,
      `${CATEGORIES.map((c, n) => `${c} ${String(before[n])} → ${String(after[n])} B`).join(", ")}, the engine's pass timer's ${String(timerBytes)} B apart; peaks while they lived: height-cache ${String(heldCache)} B, other ${String(heldOther)} B`,
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
            `view ${String(n)}: terrain ${(100 * d.cover).toFixed(1)}%, finite ${String(d.finite)}, digest ${d.digest}`,
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
 * Streams `passView` until its draw set holds every selected patch and no draft waits for its
 * normals (`TerrainPass.refining`), or for {@link CASCADES_STREAM_MS}, drawing without the cascades
 * so that SwiftShader rasterises their 2,048² layers only for the frames that need them; and steps
 * it once more. Whether it settled.
 */
async function streamedFor(
  made: TerrainPass,
  passView: TerrainPassView,
  light: TerrainLighting,
): Promise<boolean> {
  const started = performance.now();
  let settled = false;
  while (!settled && performance.now() - started < CASCADES_STREAM_MS) {
    made.step({ views: [passView], grounded: [] });
    const streaming = made.drawFor(
      passView.id,
      frameInput(passView.view, light, false, performance.now()),
    );
    // No draft waits for its normals either: the draws compared after read one slot's.
    settled =
      !streaming.missing &&
      !streaming.standingIn &&
      streaming.draws.length > 0 &&
      made.refining === 0;
    if (!settled) {
      // The workers bake and the engine answers on the event loop, between the polls.
      // oxlint-disable-next-line no-await-in-loop
      await pause(50);
    }
  }
  made.step({ views: [passView], grounded: [] });
  return settled;
}

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
  size: { readonly widthPx: number; readonly heightPx: number } = FRAME,
): Promise<Float32Array> {
  const target = engine.createRenderTarget({
    name,
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  try {
    target.render({
      label: name,
      viewRotation: viewRotation4(view.orientation),
      projection: projection(size.widthPx / size.heightPx),
      draws,
      postProcesses: [],
    });
    return halfTexels(await engine.readTexture(target.colour));
  } finally {
    target.dispose();
  }
}

/**
 * Whether an allocation is the engine's pass timer's (`webgpu/timing.ts`): its query set and its
 * resolve and read-back pairs, made at the first timed pass and kept for the engine's life.
 */
function isPassTimer(name: string): boolean {
  return name === "pass times" || name.startsWith("pass times ");
}

/** The sum of `values`. */
function sum(values: ReadonlyArray<number>): number {
  return values.reduce((a, b) => a + b, 0);
}

/** A lit mesh patch's triangles, as a caster draws it. */
const LIT_PATCH_TRIANGLES = LIT_PATCH_INDICES / 3;

/** Each cascade's caster triangles: the lit mesh's patches' and the cluster path's counts. */
function casterTriangles(
  litMesh: ReadonlyArray<number>,
  clusters: ReadonlyArray<ClusterPassStats> | null,
): number[] {
  return litMesh.map((n, k) => n * LIT_PATCH_TRIANGLES + (clusters?.[k]?.triangles ?? 0));
}

/** Each pass's clusters, for the log. */
function clustersOf(stats: ReadonlyArray<ClusterPassStats> | null): string {
  return (stats ?? []).map((each) => String(each.clusters)).join(", ");
}

/** a's sum as a share of b's, to three places. */
function shareOf(a: ReadonlyArray<number>, b: ReadonlyArray<number>): string {
  return (sum(a) / Math.max(1, sum(b))).toFixed(3);
}

/** R14.T1.d's comparison frames: large enough that 0.5% of their terrain is some hundreds. */
const COMPARISON = { widthPx: 320, heightPx: 180 } as const;

/** Each stride's clusters of each pass, blocks and strips together, for the log. */
function stridesOf(stats: ReadonlyArray<ClusterPassStats> | null): string {
  return (stats ?? []).map((each) => each.byStride.join("/")).join(", ");
}

/**
 * The high setting's geometry at stride 1, every cascade rendered every frame: its exact levers
 * alone (plan R14, R14.T1.c).
 */
const RECEIVERS_CULLED: GeometrySettings = {
  ...SETTINGS.high.geometry,
  casterStrideCap: 1,
  farCascadeRate: 1,
};

/**
 * The high setting's geometry with every cascade rendered every frame (plan R14, R14.T6.b): T1.d's
 * casters at their texel alone, and the far cascades' every-frame flight.
 */
const EVERY_FRAME: GeometrySettings = { ...SETTINGS.high.geometry, farCascadeRate: 1 };

/** R14.T6.b's flight: its frames, and the camera's step a frame, m: 300 m/s at 60 Hz. */
const FLIGHT_FRAMES = 4;
const FLIGHT_STEP_M = 300 / 60;

/**
 * How far the flight's last frame at a rate of 4 may stand from every frame's in the acne's measures
 * (plan R14, R14.T6.b), the share of its terrain's texels darker than at w = 0 and its mean lit
 * fraction: 2.5 × 10⁻⁴, some ten of the comparison's 39,379 terrain texels, against the 6 and 7 the
 * cascades darken there at 4 and every frame (both variants, SwiftShader, 2026-11-02).
 */
const ACNE_SHARE_TOLERANCE = 2.5e-4;

/**
 * The share of every frame's drawn far-map texels the held maps must hold within a texel, and the
 * fewest that must be compared, for each of cascades 2 and 3 (plan R14, R14.T6.b): 99.995% and more
 * held, of 0.96 to 2.2 million compared (both variants, SwiftShader, 2026-11-02).
 */
const FAR_MAP_WITHIN_SHARE = 0.999;
const FAR_MAP_COMPARED_LEAST = 100_000;

/** How long a flight's frame may wait for its cluster counters to come back, ms. */
const STATS_TIMEOUT_MS = 2_000;

/** The cluster path alone, at stride 1 and without the receivers' cull (plan R14, R14.T1.b). */
const CLUSTERS_ALONE: GeometrySettings = {
  ...RECEIVERS_CULLED,
  casterDepthCull: false,
};

/**
 * A smoke kernel that copies one layer of the cascades' depth into a buffer as its bits, row by
 * row, so that the layers can be fingerprinted (plan R14, R14.T1.b): a caster change that leaves
 * the frame alone still shows in them.
 */
const LAYER_COPY_KERNEL: KernelPair = {
  name: "smoke cascade layer copy",
  reference: `
@group(0) @binding(0) var layers : texture_depth_2d_array;
@group(0) @binding(1) var<storage, read_write> bits : array<u32>;
@group(0) @binding(2) var<uniform> params : vec4u;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3u) {
  let size = params.y;
  if (id.x >= size || id.y >= size) {
    return;
  }
  bits[id.y * size + id.x] = bitcast<u32>(textureLoad(layers, vec2u(id.xy), params.x, 0));
}
`,
  subgroup: null,
  readback: "bit-exact",
};

/** Each of `depth`'s `layers` layers of `sizePx`² texels, fingerprinted ({@link bytesDigest}). */
async function layerDigests(
  engine: RenderEngine,
  depth: TextureHandle,
  layers: number,
  sizePx: number,
): Promise<string[]> {
  const indices = Array.from({ length: layers }, (_, layer) => layer);
  return (await layerBits(engine, depth, indices, sizePx)).map((bits) => bytesDigest(bits));
}

/** Layers `layers` of `depth`, of `sizePx`² texels, each read back as its bits, row by row. */
async function layerBits(
  engine: RenderEngine,
  depth: TextureHandle,
  layers: ReadonlyArray<number>,
  sizePx: number,
): Promise<ArrayBuffer[]> {
  const kernel = engine.createCompute(LAYER_COPY_KERNEL);
  const bits = engine.createBuffer({
    name: "smoke cascade layer bits",
    bytes: sizePx * sizePx * 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  try {
    const read: ArrayBuffer[] = [];
    for (const layer of layers) {
      engine.dispatch(
        kernel,
        {
          uniforms: { params: Uint32Array.of(layer, sizePx, 0, 0) },
          buffers: { bits },
          sampled: { layers: depth },
          storage: {},
        },
        [Math.ceil(sizePx / 8), Math.ceil(sizePx / 8), 1],
      );
      // One layer read at a time, so that the buffer is reused.
      // oxlint-disable-next-line no-await-in-loop
      read.push(await engine.readBuffer(bits));
    }
    return read;
  } finally {
    engine.releaseBuffer(bits);
    engine.releaseCompute(kernel);
  }
}

/** A cascade's map as the frame's receivers sample it: its `lightFromCamera` and its depths. */
interface FarMap {
  readonly light: Float32Array;
  readonly depth: Float32Array;
}

/** A column-major affine 4 × 4's inverse applied to (c, 1), in `f64`. */
function unprojected(m: Float32Array, c: Xyz): Xyz {
  const at = (i: number): number => m[i] ?? Number.NaN;
  const [a, b, d, e, f, g, h, k, l] = [
    at(0),
    at(4),
    at(8),
    at(1),
    at(5),
    at(9),
    at(2),
    at(6),
    at(10),
  ];
  const x = c.x - at(12);
  const y = c.y - at(13);
  const z = c.z - at(14);
  const det = a * (f * l - g * k) - b * (e * l - g * h) + d * (e * k - f * h);
  return {
    x: ((f * l - g * k) * x - (b * l - d * k) * y + (b * g - d * f) * z) / det,
    y: (-(e * l - g * h) * x + (a * l - d * h) * y - (a * g - d * e) * z) / det,
    z: ((e * k - f * h) * x - (a * k - b * h) * y + (a * f - b * e) * z) / det,
  };
}

/** A column-major 4 × 4 applied to (p, 1), in `f64`. */
function projected(m: Float32Array, p: Xyz): Xyz {
  const at = (i: number): number => m[i] ?? Number.NaN;
  return {
    x: at(0) * p.x + at(4) * p.y + at(8) * p.z + at(12),
    y: at(1) * p.x + at(5) * p.y + at(9) * p.z + at(13),
    z: at(2) * p.x + at(6) * p.y + at(10) * p.z + at(14),
  };
}

/** The length of row `k` of a column-major 4 × 4's upper 3 × 3. */
function rowLength(m: Float32Array, k: number): number {
  return Math.hypot(m[k] ?? 0, m[k + 4] ?? 0, m[k + 8] ?? 0);
}

/**
 * How many of `fresh`'s drawn texels `held` holds within one of its texels (plan R14, R14.T6.b):
 * each drawn texel's surface point, through `fresh`'s matrix back to the camera's frame and through
 * `held`'s into its map, lies within the depths of the 3 × 3 texels about it there, widened by one
 * texel's world size in depth. So every edge `fresh` draws, a caster's against what lies behind it,
 * stands in `held` within a texel, across the light and along it. Texels within two of `held`'s
 * edge are not compared, nor a caster pancaked onto `fresh`'s near plane by `unclippedDepth`
 * (depth 1), whose place is not held: each map's near plane stands apart, and a receiver behind
 * it is shadowed in both.
 */
function heldWithinTexel(
  fresh: FarMap,
  held: FarMap,
  sizePx: number,
): { readonly compared: number; readonly found: number } {
  // One held texel, 2r ÷ N, in the held map's depth units, 1 ÷ its span.
  const slack = (2 / (sizePx * rowLength(held.light, 0))) * rowLength(held.light, 2);
  let compared = 0;
  let found = 0;
  for (let j = 0; j < sizePx; j += 1) {
    for (let i = 0; i < sizePx; i += 1) {
      const depth = fresh.depth[j * sizePx + i] ?? 0;
      if (!(depth > 0 && depth < 1)) {
        continue;
      }
      const point = unprojected(fresh.light, {
        x: (2 * (i + 0.5)) / sizePx - 1,
        y: 1 - (2 * (j + 0.5)) / sizePx,
        z: depth,
      });
      const there = projected(held.light, point);
      const hi = Math.round((0.5 + 0.5 * there.x) * sizePx - 0.5);
      const hj = Math.round((0.5 - 0.5 * there.y) * sizePx - 0.5);
      if (hi < 2 || hj < 2 || hi > sizePx - 3 || hj > sizePx - 3) {
        continue;
      }
      compared += 1;
      let low = Infinity;
      let high = -Infinity;
      for (let dj = -1; dj <= 1; dj += 1) {
        for (let di = -1; di <= 1; di += 1) {
          const near = held.depth[(hj + dj) * sizePx + hi + di] ?? 0;
          low = Math.min(low, near);
          high = Math.max(high, near);
        }
      }
      found += there.z >= low - slack && there.z <= high + slack ? 1 : 0;
    }
  }
  return { compared, found };
}

/** Each frame's rendered layers, as digits, frame by frame: "0123 01" for 4 and then 2. */
function layersAt(frames: ReadonlyArray<ReadonlyArray<number>>): string {
  return frames.map((each) => each.join("")).join(" ");
}

/** `draws` with their cascades' weight w at 0, so that they sample none. */
function atNoWeight(draws: ReadonlyArray<DrawItem>): DrawItem[] {
  return draws.map((draw) => {
    const axis = draw.uniforms["cascadeViewAxis"] ?? new Float32Array(4);
    return {
      ...draw,
      uniforms: {
        ...draw.uniforms,
        cascadeViewAxis: Float32Array.of(axis[0] ?? 0, axis[1] ?? 0, axis[2] ?? 0, 0),
      },
    };
  });
}

/**
 * The cascades' darkening of a frame against the same draws at w = 0, over its terrain by
 * luminance (plan R14, R14.T6.b): the share of its terrain's texels darker by more than
 * {@link NEVER_BRIGHTER}, and its terrain's mean lit fraction, each texel's luminance over w = 0's,
 * at most 1.
 */
function shadowShare(
  shadowed: Float32Array,
  open: Float32Array,
): { readonly darker: number; readonly darkerTexels: number; readonly litMean: number } {
  let terrain = 0;
  let darker = 0;
  let lit = 0;
  for (let p = 0; p < Math.floor(open.length / 4); p += 1) {
    if (!isTerrain(open, p)) {
      continue;
    }
    terrain += 1;
    const unlit = luminanceAt(open, p);
    const ratio = unlit > 0 ? Math.min(1, luminanceAt(shadowed, p) / unlit) : 1;
    darker += ratio < 1 - NEVER_BRIGHTER ? 1 : 0;
    lit += ratio;
  }
  return {
    darker: darker / Math.max(1, terrain),
    darkerTexels: darker,
    litMean: lit / Math.max(1, terrain),
  };
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

/**
 * R10.T9.b's cascades on the high setting: see the module's remarks. With `captures`, R14.T1.d's
 * frames at stride 1 and at the casters' texel and their terrain's mask, for a person to look at
 * and for `scripts/compareCaptures.mjs`.
 */
export async function checkShadowCascades(
  engine: RenderEngine,
  checks: Checks,
  captures = false,
): Promise<CapturedImage[]> {
  const images: CapturedImage[] = [];
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
      geometry: smokeGeometry("high"),
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
    const started = performance.now();
    const settled = await streamedFor(made, passView, light);
    // The cluster path's counters sampled (plan R14, R14.T1.b), for the casters' triangles; the
    // casters culled by their receivers' depths (R14.T1.c) at stride 1.
    made.measureClusters(true);
    made.setGeometry(RECEIVERS_CULLED);
    const frame = made.drawFor(passView.id, frameInput(view, light, true, performance.now()));
    const living = tally.liveBytes(CASCADE_MEMORY_CATEGORY) - before;
    const cascades = frame.cascades;
    const passes = cascades?.passes ?? [];
    const draws = passes.map((each) => each.draws.length);
    const weight = cascades?.set.weight ?? 0;
    checks.check(
      "R10.T9.b on the high setting near the ground the four caster passes are drawn at w > 0, the nearest with the terrain",
      frame.draws.length > 0 &&
        weight > 0 &&
        passes.length === 4 &&
        passes.every((each, n) => each.layer === n) &&
        (draws[0] ?? 0) >= 1 &&
        cascades?.set.unclippedDepth === unclipped,
      `${path}; settled ${String(settled)} in ${((performance.now() - started) / 1_000).toFixed(1)} s; w ${weight.toFixed(3)}, shadowed range ${(cascades?.set.rangeM ?? 0).toFixed(0)} m, cascade 1's texel ${((cascades?.set.cascades[0].texelM ?? 0) * 100).toFixed(2)} cm, caster reach ${((cascades?.set.casterReachM ?? 0) / 1_000).toFixed(1)} km; draws per cascade ${draws.join(", ")}`,
    );
    if (frame.draws.length === 0) {
      throw new Error("the cascades' view drew no terrain");
    }
    // The receiver's read as drawn, then the same draws at w = 0, before any other draws the set.
    const shadowed = await texelsOf(engine, "cascades: shadowed", view, frame.draws);
    const open = await texelsOf(engine, "cascades: w = 0", view, atNoWeight(frame.draws));
    const finite = shadowed.every(Number.isFinite) && open.every(Number.isFinite);
    const seen = againstUnshadowed(shadowed, open);
    const target = shadows.target;
    const layers =
      target === null
        ? ["none"]
        : await layerDigests(engine, target.depth, target.layers, target.sizePx);
    checks.check(
      "R10.T9.b the frame with cascades is finite at every texel, darker at some and nowhere brighter than at w = 0",
      finite &&
        seen.terrain >= 0.3 * FRAME.widthPx * FRAME.heightPx &&
        seen.darker > 0 &&
        seen.brighter <= NEVER_BRIGHTER,
      `${path}; finite ${String(finite)}; terrain ${String(seen.terrain)} texels, ${String(seen.darker)} darker under the cascades, the most brighter ${seen.brighter.toExponential(2)} (at most ${String(NEVER_BRIGHTER)}); digests ${bytesDigest(shadowed)} shadowed, ${bytesDigest(open)} at w = 0, layers ${layers.join(", ")}`,
    );
    // Each read waits on the device, and so on the counters' read of the frame before it.
    const culledStats = made.clusterStats(passView.id);
    // The same frame by the cluster path alone (R14.T1.b), and by the way back
    // (`GEOMETRY_BEFORE_R14`), as the pass drew it before plan R14: from the same selection and
    // residents, the cluster path's cascades' layers and frame byte for byte the way back's, and the
    // receivers' cull's frame too.
    made.setGeometry(CLUSTERS_ALONE);
    const alone = made.drawFor(passView.id, frameInput(view, light, true, performance.now()));
    const shadowedAlone = await texelsOf(engine, "cascades: clusters alone", view, alone.draws);
    const layersAlone =
      target === null
        ? ["none"]
        : await layerDigests(engine, target.depth, target.layers, target.sizePx);
    const aloneStats = made.clusterStats(passView.id);
    made.setGeometry(GEOMETRY_BEFORE_R14);
    const unlevered = made.drawFor(passView.id, frameInput(view, light, true, performance.now()));
    const shadowedBefore = await texelsOf(engine, "cascades: before R14", view, unlevered.draws);
    const layersBefore =
      target === null
        ? ["none"]
        : await layerDigests(engine, target.depth, target.layers, target.sizePx);
    made.setGeometry(SETTINGS.high.geometry);
    const casting = casterTriangles(alone.cascades?.litMeshCasters ?? [], aloneStats);
    const culled = casterTriangles(cascades?.litMeshCasters ?? [], culledStats);
    const prior = casterTriangles(unlevered.cascades?.litMeshCasters ?? [], null);
    checks.check(
      "R14.T1 the cascades' layers and the frame they shadow are those the pass drew before plan R14, byte for byte",
      bytesDigest(shadowedBefore) === bytesDigest(shadowedAlone) &&
        layersBefore.join() === layersAlone.join() &&
        alone.cascades?.clustered === true &&
        aloneStats !== null,
      `${path}; ${String(alone.patches.interior)} interior and ${String(alone.patches.edge)} edge patches, the interior cast by clusters ${String(alone.cascades?.clustered)}; caster triangles by cascade ${casting.join(", ")} (clusters ${clustersOf(aloneStats)}), ${prior.join(", ")} before, ${shareOf(casting, prior)} of them; digests ${bytesDigest(shadowedAlone)}, ${bytesDigest(shadowedBefore)} before; layers ${layersAlone.join(", ")}; before ${layersBefore.join(", ")}`,
    );
    checks.check(
      "R14.T1.c the casters culled by their receivers' depths shadow the frame the pass drew before plan R14, byte for byte, with fewer caster triangles",
      bytesDigest(shadowedBefore) === bytesDigest(shadowed) &&
        cascades?.clustered === true &&
        culledStats !== null &&
        culled.every((n, k) => n <= (casting[k] ?? 0)) &&
        sum(culled) < sum(casting),
      `${path}; caster triangles by cascade ${culled.join(", ")} (clusters ${clustersOf(culledStats)}), ${shareOf(culled, casting)} of the cluster path's alone, ${shareOf(culled, prior)} of before R14's; digests ${bytesDigest(shadowed)}, ${bytesDigest(shadowedBefore)} before; layers ${layers.join(", ")}, ${layers.filter((d, k) => d !== layersBefore[k]).length} of them differing where no receiver samples`,
    );
    // R14.T1.d: the casters at their cascade's texel against T1.c's at stride 1, from the same
    // selection and residents at the same exposure, at the comparison's size: bounded, not exact.
    made.setGeometry(RECEIVERS_CULLED);
    const exact = made.drawFor(passView.id, frameInput(view, light, true, performance.now()));
    const exactTexels = await texelsOf(engine, "cascades: stride 1", view, exact.draws, COMPARISON);
    const exactStats = made.clusterStats(passView.id);
    made.setGeometry(EVERY_FRAME);
    const strided = made.drawFor(passView.id, frameInput(view, light, true, performance.now()));
    const stridedTexels = await texelsOf(
      engine,
      "cascades: at the texel",
      view,
      strided.draws,
      COMPARISON,
    );
    const stridedStats = made.clusterStats(passView.id);
    const exactTriangles = casterTriangles(exact.cascades?.litMeshCasters ?? [], exactStats);
    const stridedTriangles = casterTriangles(strided.cascades?.litMeshCasters ?? [], stridedStats);
    const apart = stopsApart(exactTexels, stridedTexels);
    const share = apart.within / Math.max(1, apart.terrain);
    // Where no cluster qualifies for a stride (each patch's spacing above half of every texel it
    // casts into, as the fixture's smooth fine levels leave its selection), the frame is stride 1's.
    const anyStrided = (stridedStats ?? []).some((each) =>
      each.byStride.some((n, l) => l > 0 && n > 0),
    );
    const unmoved = bytesDigest(exactTexels) === bytesDigest(stridedTexels);
    checks.check(
      "R14.T1.d the casters at their cascade's texel shadow the frame within a third of a stop of stride 1's at 99.5% of its terrain texels, with fewer caster triangles where any cluster strides",
      apart.terrain > 0.3 * COMPARISON.widthPx * COMPARISON.heightPx &&
        share >= 0.995 &&
        stridedStats !== null &&
        (anyStrided ? sum(stridedTriangles) < sum(exactTriangles) : unmoved),
      `${path}; ${anyStrided ? "clusters strided" : "no cluster qualifies for a stride, and the frame is stride 1's"}; ${String(strided.patches.interior)} interior and ${String(strided.patches.edge)} edge patches; ${String(apart.terrain)} terrain texels, ${String(apart.differing)} differing, ${(100 * share).toFixed(2)}% within a third of a stop, the rest ${String(apart.terrain - apart.within)}, the largest ${apart.largestStops.toFixed(3)} stops; caster triangles by cascade ${stridedTriangles.join(", ")}, ${shareOf(stridedTriangles, exactTriangles)} of stride 1's ${exactTriangles.join(", ")}, ${shareOf(stridedTriangles, prior)} of before R14's; clusters by stride 1/2/4/8 ${stridesOf(stridedStats)}; digests ${bytesDigest(exactTexels)}, ${bytesDigest(stridedTexels)}`,
    );
    if (captures) {
      // The exposure fixed by stride 1's brightest terrain, so that the two compare as they are.
      const scale = captureScale(exactTexels);
      images.push(
        captureOf("r14-t1d-casters-stride-1", exactTexels, COMPARISON, scale),
        captureOf("r14-t1d-casters-at-texel", stridedTexels, COMPARISON, scale),
        maskOf("r14-t1d-terrain-mask", exactTexels, COMPARISON),
      );
    }
    // R14.T6.b: the far cascades at the high setting's rate of 4 against every frame, over the same
    // frames from the same selection and residents: the camera flown west at R05's descent's
    // 300 m/s, and the sun setting a quarter of the holds' share of its disc a frame, so that the
    // last frame samples cascade 2 as the first drew it, 15 m back under a sun three quarters of the
    // share higher, and cascade 3 as the third did. Its frame at the comparison's size, with T1.c's
    // cull as the setting has it; and its far maps against every frame's, texel by texel, with the
    // cull off in both, which otherwise leaves the layers apart where no receiver samples them.
    const star = light.lights[0];
    const discRad =
      star === undefined ? 0 : 2 * Math.asin(Math.min(1, star.radius / star.distance));
    const stepRad = (FAR_CASCADE_SUN_SHARE * discRad) / 4;
    const { east, north } = frameOverCentre();
    const headingRad = (CASCADES_HEADING_DEG * Math.PI) / 180;
    const ahead = along(north, Math.cos(headingRad), east, Math.sin(headingRad));
    const fly = async (
      geometry: GeometrySettings,
      read: "frame" | "far maps",
    ): Promise<{
      readonly layers: number[][];
      readonly triangles: number[];
      readonly shadowed: Float32Array | null;
      readonly open: Float32Array | null;
      readonly far: readonly [FarMap, FarMap] | null;
    }> => {
      made.setGeometry(geometry);
      const flownLayers: number[][] = [];
      const triangles: number[] = [];
      let at = view;
      let lastDraws: ReadonlyArray<DrawItem> = [];
      for (let k = 0; k < FLIGHT_FRAMES; k += 1) {
        at = viewFrom(
          along(view.cameraM, 1, ahead, FLIGHT_STEP_M * k),
          CASCADES_DEPRESSION_DEG,
          CASCADES_HEADING_DEG,
        );
        const sampled = made.clusterStats(passView.id);
        const flown = made.drawFor(
          passView.id,
          frameInput(at, lighting(CASCADES_SUN_RAD - k * stepRad), true, performance.now()),
        );
        flownLayers.push((flown.cascades?.passes ?? []).map((each) => each.layer));
        // The frame's own counters, read back before the next frame.
        // oxlint-disable-next-line no-await-in-loop
        await until(() => made.clusterStats(passView.id) !== sampled, STATS_TIMEOUT_MS);
        const stats = made.clusterStats(passView.id);
        triangles.push(sum(casterTriangles(flown.cascades?.litMeshCasters ?? [], stats)));
        lastDraws = flown.draws;
      }
      const name = `cascades: flown at ${String(geometry.farCascadeRate)}`;
      if (read === "frame") {
        // The receiver's read as drawn, and then at w = 0, before the next frame writes its fields.
        return {
          layers: flownLayers,
          triangles,
          shadowed: await texelsOf(engine, name, at, lastDraws, COMPARISON),
          open: await texelsOf(engine, `${name}, w = 0`, at, atNoWeight(lastDraws), COMPARISON),
          far: null,
        };
      }
      const uniforms = lastDraws.at(-1)?.uniforms ?? {};
      const bits =
        target === null ? [] : await layerBits(engine, target.depth, [2, 3], target.sizePx);
      const mapOf = (n: 0 | 1): FarMap => ({
        light: Float32Array.from(uniforms[`cascadeLight${String(n + 2)}`] ?? []),
        depth: new Float32Array(bits[n] ?? new ArrayBuffer(0)),
      });
      return {
        layers: flownLayers,
        triangles,
        shadowed: null,
        open: null,
        far: [mapOf(0), mapOf(1)],
      };
    };
    const everyFrame = await fly(EVERY_FRAME, "frame");
    const atFour = await fly(SETTINGS.high.geometry, "frame");
    const everyFrameMaps = await fly({ ...EVERY_FRAME, casterDepthCull: false }, "far maps");
    const atFourMaps = await fly({ ...SETTINGS.high.geometry, casterDepthCull: false }, "far maps");
    made.setGeometry(SETTINGS.high.geometry);
    const everyShadowed = everyFrame.shadowed ?? new Float32Array(0);
    const fourShadowed = atFour.shadowed ?? new Float32Array(0);
    const edges = stopsApart(everyShadowed, fourShadowed);
    const edgeShare = edges.within / Math.max(1, edges.terrain);
    const acneEvery = shadowShare(everyShadowed, everyFrame.open ?? new Float32Array(0));
    const acneAtFour = shadowShare(fourShadowed, atFour.open ?? new Float32Array(0));
    const cascadeSize = shadows.target?.sizePx ?? 0;
    const within = ([0, 1] as const).map((n) => {
      const fresh = everyFrameMaps.far?.[n];
      const kept = atFourMaps.far?.[n];
      return fresh === undefined || kept === undefined
        ? { compared: 0, found: 0 }
        : heldWithinTexel(fresh, kept, cascadeSize);
    });
    const withinShare = within.map(({ compared, found }) => found / Math.max(1, compared));
    const trianglesOf = (triangles: ReadonlyArray<number>): string =>
      `${triangles.join(", ")} (${String(sum(triangles))})`;
    checks.check(
      "R14.T6.b the far cascades at a rate of 4 render 4, 2, 3 and 2 passes over the descent's speed and a setting sun, hold every surface of every frame's far maps within one of their texels, and shadow the frame within a third of a stop of every frame's at 99.5% of its terrain texels, its acne unchanged",
      layersAt(everyFrame.layers) === "0123 0123 0123 0123" &&
        layersAt(atFour.layers) === "0123 01 013 01" &&
        layersAt(atFourMaps.layers) === "0123 01 013 01" &&
        within.every(({ compared }) => compared > FAR_MAP_COMPARED_LEAST) &&
        withinShare.every((each) => each >= FAR_MAP_WITHIN_SHARE) &&
        edges.terrain > 0.3 * COMPARISON.widthPx * COMPARISON.heightPx &&
        edgeShare >= 0.995 &&
        Math.abs(acneAtFour.darker - acneEvery.darker) <= ACNE_SHARE_TOLERANCE &&
        Math.abs(acneAtFour.litMean - acneEvery.litMean) <= ACNE_SHARE_TOLERANCE,
      `${path}; layers by frame ${layersAt(atFour.layers)} at 4, ${layersAt(everyFrame.layers)} every frame; the camera ${String(FLIGHT_STEP_M)} m a frame, the sun ${(stepRad * 1e6).toFixed(1)} µrad a frame; far maps held within a texel: cascade 2 ${String(within[0]?.found)} of ${String(within[0]?.compared)} texels, cascade 3 ${String(within[1]?.found)} of ${String(within[1]?.compared)}; caster triangles by frame ${trianglesOf(atFour.triangles)} at 4, ${trianglesOf(everyFrame.triangles)} every frame; ${String(edges.terrain)} terrain texels, ${String(edges.differing)} differing, ${(100 * edgeShare).toFixed(2)}% within a third of a stop, the largest ${edges.largestStops.toFixed(3)} stops; darker than at w = 0 ${String(acneAtFour.darkerTexels)} texels at 4 and ${String(acneEvery.darkerTexels)} every frame, mean lit fraction ${acneAtFour.litMean.toFixed(6)} and ${acneEvery.litMean.toFixed(6)}; digests ${bytesDigest(everyShadowed)}, ${bytesDigest(fourShadowed)}`,
    );
    if (captures) {
      const scale = captureScale(everyShadowed);
      images.push(
        captureOf("r14-t6b-every-frame", everyShadowed, COMPARISON, scale),
        captureOf("r14-t6b-far-at-4", fourShadowed, COMPARISON, scale),
        maskOf("r14-t6b-terrain-mask", everyShadowed, COMPARISON),
      );
    }
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
  return images;
}
