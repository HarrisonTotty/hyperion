/**
 * The smoke harness's terrain checks (plan R05, R05.T11.a and T11.b): the terrain's GPU resources
 * made on the real device at both settings, the high setting's `FaceDifferences` fallback on a
 * second engine that asks for WebGPU's default limits (decisions-r06-r07.md item 7), and frames of
 * the test planet drawn by both vertex paths from patches a height worker bakes.
 *
 * @remarks
 * A buffer or texture beyond the device's limits, or a write outside one, raises an uncaptured
 * validation error, which the harness's main process counts against the run, so these checks pass
 * only if the layouts fit the devices they are made on.
 */

import { lookAlong, quaternionFromRows } from "../view/camera/quaternion";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  viewRotation4,
} from "../view/camera/projection";
import { goldenLevelTable, WGS84_FIGURE } from "../test/terrainFixtures";
import { sunIlluminanceRgb } from "../view/atmosphere/solar";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import { loadRenderEngine } from "../view/engine/loadEngine";
import { LitView } from "../view/spike/litView";
import { planetGeometry } from "../view/terrain/planet";
import { type TerrainFrame, TerrainPass } from "../view/terrain/terrainPass";
import { HeightWorkerPool } from "../view/terrain/workers/pool";
import { base64Of, type CapturedImage, srgb8 } from "./atmosphere";
import { requestAdapterOutcome } from "../view/engine/platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "../view/engine/status";
import type { BufferHandle, RenderEngine, RenderTarget } from "../view/engine/types";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { terrainMaterialSpec } from "../view/terrain/gpu/material";
import { TerrainResources } from "../view/terrain/gpu/resources";
import { ContactRecords, InstanceRecords, patchTerms } from "../view/terrain/gpu/uniforms";
import type { PatchKey } from "../view/terrain/patchKey";
import type {
  BakedPatch,
  BakeSettings,
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../view/terrain/workers/messages";
import { HEIGHTS_BYTES, OFFSETS_BYTES } from "../view/terrain/slotLayout";
import { type Checks, halfTexels, pause, projection } from "./harness";

/** WGS 84 (NIMA TR8350.2): a, and c = a (1 − f) with 1 ÷ f = 298.257223563. */
const WGS84 = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563) };

/** WebGPU's default `maxStorageBufferBindingSize`, 128 MiB (W3C WebGPU §3.6.2). */
const DEFAULT_STORAGE_BINDING_BYTES = 134_217_728;

/** Makes `setting`'s resources on `engine`, writes its first and last slots and one frame. */
function exercise(engine: RenderEngine, setting: "high" | "low"): TerrainResources {
  const resources = new TerrainResources(engine, TERRAIN_SETTINGS[setting], WGS84);
  const { slots, atlas, vertexPath } = resources.layout;
  const n = atlas.samplesPerSide;
  for (const slot of [0, slots.slotCount - 1]) {
    resources.upload({
      slot,
      key: { face: 1, level: 10, i: 512, j: 300 },
      heights: new Float32Array(HEIGHTS_BYTES / 4).fill(100),
      offsets: vertexPath === "baked-offsets" ? new Float32Array(OFFSETS_BYTES / 4) : null,
      normals: new Float16Array(n * n * 2),
      originHeightM: 100,
      skirtDepthM: 2,
    });
  }
  const instances = new InstanceRecords(slots.slotCount);
  instances.push(slots.slotCount - 1, { x: 0, y: 0, z: -1000 }, 500, 1000);
  const contacts = new ContactRecords();
  contacts.push({ x: 0, y: 0, z: -10 }, 30, 17.7);
  resources.writeFrame(instances, contacts);
  return resources;
}

/** Describes a layout for a check's detail. */
function describe(resources: TerrainResources): string {
  const { slots, atlas, vertexPath, fallback } = resources.layout;
  return `${vertexPath} (fallback ${fallback}), ${slots.slotCount} slots, atlas ${atlas.widthTexels} × ${atlas.heightTexels} × ${atlas.layers}`;
}

/** The terrain's resources on `engine`, then the fallback on an engine with default limits. */
export async function checkTerrainResources(engine: RenderEngine, checks: Checks): Promise<void> {
  const binding = engine.capabilities.maxStorageBufferBindingSize;
  for (const setting of ["high", "low"] as const) {
    const resources = exercise(engine, setting);
    const offsets = resources.field("offsets");
    checks.check(
      `R05.T11.a the ${setting} setting's resources fit the device`,
      (offsets?.bytes ?? 0) <= binding &&
        (resources.field("heights")?.bytes ?? Number.POSITIVE_INFINITY) <= binding,
      `${describe(resources)}; binding ${binding} B`,
    );
    resources.dispose();
  }

  const outcome = await requestAdapterOutcome(navigator.gpu);
  if (outcome.kind !== "adapter") {
    throw new Error(`no adapter for the default-limits engine: ${outcome.kind}`);
  }
  const limited = await loadRenderEngine(
    outcome,
    new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
    { overrides: { withholdSubgroups: false, withholdShaderF16: false, defaultLimits: true } },
  );
  try {
    checks.check(
      "R05.T11.a the default-limits engine has WebGPU's 128 MiB binding",
      limited.capabilities.maxStorageBufferBindingSize === DEFAULT_STORAGE_BINDING_BYTES,
      `${limited.capabilities.maxStorageBufferBindingSize} B`,
    );
    const resources = exercise(limited, "high");
    checks.check(
      "R05.T11.a under default limits the high setting falls back to FaceDifferences",
      resources.layout.vertexPath === "face-differences" &&
        resources.layout.fallback === "binding-limit" &&
        resources.field("offsets") === null &&
        (resources.field("heights")?.bytes ?? Number.POSITIVE_INFINITY) <=
          DEFAULT_STORAGE_BINDING_BYTES,
      describe(resources),
    );
    resources.dispose();
    // Uncaptured errors arrive as events: let them reach the page's log before the device goes.
    await pause(250);
  } finally {
    limited.dispose();
  }
}

/** The frames' size, pixels. */
const FRAME = { widthPx: 64, heightPx: 48 } as const;

/** The frames' cache budget, 64 MiB: a few hundred slots on either path. */
const FRAMES_CACHE_BYTES = 2 ** 26;

/** How long the worker may take over a frame's bakes, milliseconds. */
const BAKES_TIMEOUT_MS = 120_000;

/** One frame of the check: the patches to bake, and where the camera is and looks. */
interface TerrainShot {
  readonly name: string;
  /** The patch the camera stands over; the rest surround it. */
  readonly centre: PatchKey;
  /** Cells on each side of the centre, a (2r + 1)² block. */
  readonly radius: number;
  /** The camera's height above the centre patch's origin, along its normal, metres. */
  readonly heightM: number;
  /** The view's angle below the horizon, degrees: 90 looks straight down. */
  readonly depressionDeg: number;
  /** The least share of the frame the terrain must cover. */
  readonly minCover: number;
}

/**
 * The plan's two views (T11.b's tests): from 400 km straight down over a 4 × 4 block of level-5
 * patches about 300 km across, and from 10 m, 30° below the horizon, over a 5 × 5 block of level-18
 * patches about 35 m across.
 */
const SHOTS: ReadonlyArray<TerrainShot> = [
  {
    name: "400 km",
    centre: { face: 2, level: 5, i: 16, j: 16 },
    radius: 2,
    heightM: 400_000,
    depressionDeg: 90,
    minCover: 0.9,
  },
  {
    name: "10 m",
    centre: { face: 2, level: 18, i: 131_072, j: 131_072 },
    radius: 2,
    heightM: 10,
    depressionDeg: 30,
    minCover: 0.25,
  },
];

/** Bakes `keys` in a fresh height worker, in order. */
async function bakeAll(
  keys: ReadonlyArray<PatchKey>,
  settings: BakeSettings,
): Promise<BakedPatch[]> {
  const worker = new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
    type: "module",
  });
  try {
    return await new Promise<BakedPatch[]>((resolve, reject) => {
      const baked = new Map<number, BakedPatch>();
      const timer = setTimeout(() => {
        reject(new Error(`${baked.size} of ${keys.length} bakes within ${BAKES_TIMEOUT_MS} ms`));
      }, BAKES_TIMEOUT_MS);
      worker.addEventListener("message", (event: MessageEvent<HeightWorkerReply>) => {
        const reply = event.data;
        if (reply.kind !== "baked") {
          clearTimeout(timer);
          reject(new Error(`the worker answered ${JSON.stringify(reply)}`));
          return;
        }
        baked.set(reply.id, reply.bake);
        if (baked.size === keys.length) {
          clearTimeout(timer);
          resolve(keys.map((_, id) => baked.get(id)).filter((b) => b !== undefined));
        }
      });
      worker.addEventListener("error", (event: Event) => {
        clearTimeout(timer);
        reject(new Error(event instanceof ErrorEvent ? event.message : "the worker did not load"));
      });
      keys.forEach((key, id) => {
        const request: HeightWorkerRequest = { kind: "bake", id, key, generation: 1, settings };
        worker.postMessage(request, []);
      });
    });
  } finally {
    worker.terminate();
  }
}

/** The (2r + 1)² block of keys about `centre`, the centre first. */
function block(centre: PatchKey, radius: number): PatchKey[] {
  const keys: PatchKey[] = [centre];
  for (let dj = -radius; dj <= radius; dj += 1) {
    for (let di = -radius; di <= radius; di += 1) {
      if (di !== 0 || dj !== 0) {
        keys.push({ ...centre, i: centre.i + di, j: centre.j + dj });
      }
    }
  }
  return keys;
}

interface Xyz {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

function cross(a: Xyz, b: Xyz): Xyz {
  return { x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x };
}

/** a ka + b kb. */
function along(a: Xyz, ka: number, b: Xyz, kb: number): Xyz {
  return { x: a.x * ka + b.x * kb, y: a.y * ka + b.y * kb, z: a.z * ka + b.z * kb };
}

function unit(v: Xyz): Xyz {
  const n = Math.hypot(v.x, v.y, v.z);
  return { x: v.x / n, y: v.y / n, z: v.z / n };
}

/** Draws `shot` with `setting`'s path and returns its texels and what share is terrain. */
async function drawShot(
  engine: RenderEngine,
  setting: "high" | "low",
  shot: TerrainShot,
): Promise<{ readonly finite: boolean; readonly cover: number; readonly detail: string }> {
  // The setting's path and normals over a small cache: the frames draw 25 patches at most, and the
  // engine frees nothing before it is disposed.
  const settings = { ...TERRAIN_SETTINGS[setting], cacheBytes: FRAMES_CACHE_BYTES };
  const resources = new TerrainResources(engine, settings, WGS84);
  const { layout } = resources;
  const keys = block(shot.centre, shot.radius);
  const bakes = await bakeAll(keys, {
    vertexPath: layout.vertexPath,
    normals: TERRAIN_SETTINGS[setting].normals,
    ridges: "off",
  });
  bakes.forEach((bake, slot) => {
    const result = resources.upload({ slot, ...bake });
    if (result.kind !== "uploaded") {
      throw new Error(`slot ${slot} refused: ${result.reason}`);
    }
  });
  const centre = bakes[0];
  if (centre === undefined) {
    throw new Error("no centre patch was baked");
  }
  // The camera above the centre's origin along its spheroid normal, in body-fixed f64 metres; the
  // body's axes are the frame's (no rotation).
  const nu = patchTerms(centre.key, WGS84, centre.originHeightM).nu0;
  const camera = {
    x: centre.originM.x + shot.heightM * nu.x,
    y: centre.originM.y + shot.heightM * nu.y,
    z: centre.originM.z + shot.heightM * nu.z,
  };
  // Two tangent directions; the reference axis avoids the pole, where face 2's centre lies.
  const reference = Math.abs(nu.z) > 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 0, z: 1 };
  const east = unit(cross(reference, nu));
  const north = cross(nu, east);
  const d = (shot.depressionDeg * Math.PI) / 180;
  const forward = along(north, Math.cos(d), nu, -Math.sin(d));
  const upUnit = along(north, Math.sin(d), nu, Math.cos(d));
  const right = cross(forward, upUnit);
  const orientation = quaternionFromRows([
    { x: right.x, y: upUnit.x, z: -forward.x },
    { x: right.y, y: upUnit.y, z: -forward.y },
    { x: right.z, y: upUnit.z, z: -forward.z },
  ]);
  const instances = new InstanceRecords(layout.slots.slotCount);
  bakes.forEach((bake, slot) => {
    instances.push(
      slot,
      {
        x: bake.originM.x - camera.x,
        y: bake.originM.y - camera.y,
        z: bake.originM.z - camera.z,
      },
      0,
      0,
    );
  });
  const contacts = new ContactRecords();
  contacts.push(
    { x: -shot.heightM * nu.x, y: -shot.heightM * nu.y, z: -shot.heightM * nu.z },
    5,
    17.7,
  );
  resources.writeFrame(instances, contacts);

  const material = await engine.createMaterialAsync(
    terrainMaterialSpec(layout.vertexPath),
    ["rgba16float"],
    [resources.mesh],
  );
  const target = engine.createRenderTarget({
    name: `terrain ${shot.name} ${setting}`,
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const sun = unit({ x: nu.x + 0.5 * east.x, y: nu.y + 0.5 * east.y, z: nu.z + 0.5 * east.z });
  const storageBuffers: Record<string, BufferHandle> = {
    slots: resources.slotRecords,
    instances: resources.instances,
    contacts: resources.contacts,
  };
  for (const name of ["heights", "offsets"] as const) {
    const buffer = resources.field(name);
    if (buffer !== null) {
      storageBuffers[name] = buffer;
    }
  }
  const atlas = layout.atlas;
  target.render({
    label: "terrain",
    viewRotation: viewRotation4(orientation),
    projection: projection(FRAME.widthPx / FRAME.heightPx),
    draws: [
      {
        mesh: resources.mesh,
        material,
        offsetFromCameraM: new Float32Array(3),
        uniforms: {
          bodyRotation: new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
          sunDirection: new Float32Array([sun.x, sun.y, sun.z, 0]),
          sunRadiance: new Float32Array([1, 1, 1, 0]),
          atlas: new Float32Array([
            atlas.columns,
            atlas.tilesPerLayer,
            atlas.tileTexels,
            atlas.samplesPerSide,
          ]),
        },
        textures: { normals: resources.normals },
        storageBuffers,
        indirect: { buffer: resources.indirect, offsetBytes: 0 },
      },
    ],
    postProcesses: [],
  });
  const texels = halfTexels(await engine.readTexture(target.colour));
  let terrain = 0;
  let lit = 0;
  for (let p = 0; p < FRAME.widthPx * FRAME.heightPx; p += 1) {
    if (texels[4 * p + 3] === 2) {
      terrain += 1;
      if ((texels[4 * p] ?? 0) > 0) {
        lit += 1;
      }
    }
  }
  target.dispose();
  resources.dispose();
  const cover = terrain / (FRAME.widthPx * FRAME.heightPx);
  return {
    finite: texels.every(Number.isFinite),
    cover,
    detail: `${layout.vertexPath}: ${bakes.length} patches, terrain ${(100 * cover).toFixed(1)}% of the frame, ${lit} lit pixels`,
  };
}

/** T11.b's frames: the test planet from 400 km and from 10 m, by both vertex paths. */
export async function checkTerrainFrames(engine: RenderEngine, checks: Checks): Promise<void> {
  for (const shot of SHOTS) {
    for (const setting of ["high", "low"] as const) {
      // The frames run in order: each bakes, draws and reads back before the next allocates.
      // oxlint-disable-next-line no-await-in-loop
      const { finite, cover, detail } = await drawShot(engine, setting, shot);
      checks.check(
        `R05.T11.b the test planet from ${shot.name} (${setting}) is finite and drawn`,
        finite && cover >= shot.minCover,
        `${detail}; finite ${String(finite)}, at least ${(100 * shot.minCover).toFixed(0)}% wanted`,
      );
    }
  }
}

// --- R05.T11.c: the terrain captures ----------------------------------------------------------------

/** The captures' size, pixels. */
const CAPTURE = { widthPx: 480, heightPx: 270 } as const;

/** The longest the captures wait for their patches, milliseconds. */
const SETTLE_MS = 120_000;

/** How long with no new resident patch counts as settled, milliseconds. */
const QUIET_MS = 3_000;

/** One capture: a camera over the edge between faces 0 (+x) and 2 (+z), looking along it. */
interface CaptureShot {
  readonly name: string;
  readonly setting: "high" | "low";
  readonly heightM: number;
  /** Degrees below the horizon. */
  readonly depressionDeg: number;
}

/**
 * The plan's two looks (T11.c): from 400 km and from 2 m, each over the cube's +x/+z face edge, so
 * that a crack along it would show, and tilted below the horizon, so that up is plain.
 */
const CAPTURES: ReadonlyArray<CaptureShot> = [
  { name: "terrain-400km", setting: "high", heightM: 400_000, depressionDeg: 30 },
  { name: "terrain-2m", setting: "low", heightM: 2, depressionDeg: 10 },
];

/**
 * R05.T11.c's captures: the test planet drawn by the terrain pass, streamed by a height-worker
 * pool, through the lit view's AgX, read back and encoded as 8-bit sRGB, for the lane to look at
 * (the right way up, no crack along a face edge) and for the owner's on-screen look.
 */
export async function captureTerrain(
  engine: RenderEngine,
  checks: Checks,
): Promise<CapturedImage[]> {
  const images: CapturedImage[] = [];
  for (const shot of CAPTURES) {
    // The captures run in order: each streams, draws and reads back before the next.
    // oxlint-disable-next-line no-await-in-loop
    images.push(await captureShot(engine, shot, checks));
  }
  return images;
}

async function captureShot(
  engine: RenderEngine,
  shot: CaptureShot,
  checks: Checks,
): Promise<CapturedImage> {
  const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
  const pass = new TerrainPass({
    engine,
    setting: shot.setting,
    planet,
    ridges: "off",
    createPool: (bake) =>
      new HeightWorkerPool({
        workers: 3,
        bake,
        createWorker: () =>
          new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
            type: "module",
          }),
      }),
  });
  let lit: LitView | null = null;
  let output: RenderTarget | null = null;
  try {
    lit = new LitView(engine, shot.name, CAPTURE, TERRAIN_SETTINGS[shot.setting].renderHeightPx);
    output = engine.createRenderTarget({
      name: `${shot.name}:display`,
      size: CAPTURE,
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    await Promise.all([pass.ready(), lit.ready(["rgba16float"])]);
    // The camera over the face edge, on the datum's normal there; the body does not turn.
    const dir = unit({ x: 1, y: 0.05, z: 1 });
    // The datum's point M·d and its normal M⁻¹d ÷ |M⁻¹d| (Design note 5).
    const { equatorialRadiusM: a, polarRadiusM: c } = planet.figure;
    const surface = { x: a * dir.x, y: a * dir.y, z: c * dir.z };
    const nu = unit({ x: dir.x / a, y: dir.y / a, z: dir.z / c });
    const cameraM = along(surface, 1, nu, shot.heightM);
    const east = unit(cross({ x: 0, y: 0, z: 1 }, nu));
    const north = cross(nu, east);
    // Looking along the edge's direction (−y is along the +x/+z edge at y = 0), tilted down.
    const ahead = unit(along(east, -1, north, 0));
    const d = (shot.depressionDeg * Math.PI) / 180;
    const forward = along(ahead, Math.cos(d), nu, -Math.sin(d));
    const up = along(ahead, Math.sin(d), nu, Math.cos(d));
    const orientation = lookAlong(forward, up);
    const view = {
      rotation: IDENTITY_ROTATION,
      cameraM,
      orientation,
      fovXRad: Math.PI / 3,
      // Selection and the terrain see the render size, which the display pass upscales.
      viewport: lit.renderSize,
    };
    const projectionMatrix = perspectiveReversedInfinite(
      view.fovXRad,
      CAPTURE.widthPx / CAPTURE.heightPx,
      NEAR_PLANE_M,
    );
    const viewRotation = viewRotation4(orientation);
    const sun = unit(along(nu, 1, ahead, 0.6));
    const started = performance.now();
    let lastResident = started;
    let resident = -1;
    let frame: TerrainFrame | null = null;
    let settled = false;
    for (;;) {
      const now = performance.now();
      frame = pass.frame({
        view,
        grounded: [],
        sunDirectionBodyFixed: sun,
        sunIlluminanceLx: sunIlluminanceRgb(),
        exposureScale: lit.exposureScale,
        nowMs: now,
      });
      const drawn = frame.drawSet.patches.length - frame.drawSet.standingIn;
      if (drawn !== resident) {
        resident = drawn;
        lastResident = now;
      }
      settled = frame.drawSet.standingIn === 0 && frame.drawSet.missing === 0;
      if (settled || now - lastResident > QUIET_MS || now - started > SETTLE_MS) {
        break;
      }
      // Streaming runs in the workers; give them the event loop between frames.
      // oxlint-disable-next-line no-await-in-loop
      await pause(50);
    }
    lit.render(output, viewRotation, projectionMatrix, frame.draw === null ? [] : [frame.draw]);
    const texels = halfTexels(await engine.readTexture(output.colour));
    const rgba = new Uint8Array(texels.length);
    for (let i = 0; i < texels.length; i += 4) {
      rgba[i] = srgb8(texels[i] ?? 0);
      rgba[i + 1] = srgb8(texels[i + 1] ?? 0);
      rgba[i + 2] = srgb8(texels[i + 2] ?? 0);
      rgba[i + 3] = 255;
    }
    checks.check(
      `R05.T11.c the ${shot.name} capture streamed its patches`,
      settled,
      `${frame.selection.patches.size} selected, ${frame.drawSet.patches.length} drawn, ` +
        `${frame.drawSet.standingIn} standing in, ${frame.drawSet.missing} missing, ` +
        `limited ${String(frame.selection.limited)}, after ` +
        `${Math.round(performance.now() - started)} ms (${shot.setting}, render ` +
        `${lit.renderSize.widthPx} × ${lit.renderSize.heightPx})`,
    );
    return {
      name: shot.name,
      width: CAPTURE.widthPx,
      height: CAPTURE.heightPx,
      rgba: base64Of(rgba),
    };
  } finally {
    output?.dispose();
    lit?.dispose();
    pass.dispose();
  }
}
