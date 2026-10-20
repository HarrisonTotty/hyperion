/**
 * The smoke harness's lit-terrain check (plan R10, R10.T10.b): a generated world's patch set drawn
 * by R10's lit material on the real device, by both vertex paths.
 *
 * @remarks
 * The world is `hyperion_surface`'s Moon-like synthetic world, its first survey's payload blocks
 * as the worker fixture pins them (`tests/golden/worker/moon_like.golden`), which a height worker
 * takes into its field and bakes a 3 × 3 block of level-7 patches over, with their class weights,
 * their palette's photometry and their horizon maps, and their four parents, whose maps the block's
 * shadows blend to by a morph factor of about ¼ to ½ (R10.T8.b). They are drawn from 30 km above
 * the block, 60° below the horizon, under one Sun-like star and one planetshine source through a
 * body without air. The
 * frame must be finite at every texel, mostly terrain, and lit. The block lies within the survey,
 * so that R10.T10.c's discard keeps all of it. A buffer or texture beyond the
 * device's limits, or a write outside one, raises an uncaptured validation error, which the
 * harness's main process counts against the run.
 */

import { withTemplateCurve } from "../view/appearance/phase";
import {
  createAirlessTables,
  surfaceLightingDrawTextures,
  type SurfaceLightingTables,
} from "../view/atmosphere/surfaceLighting";
import type { Quaternion } from "../view/camera/pose";
import { viewRotation4 } from "../view/camera/projection";
import { quaternionFromRows } from "../view/camera/quaternion";
import type {
  BufferHandle,
  MaterialHandle,
  RenderEngine,
  RenderTarget,
  TextureHandle,
} from "../view/engine/types";
import { annulusEdges } from "../view/lighting/annuli";
import { TERRAIN_SETTINGS } from "../view/quality/qualitySetting";
import { normalsAtlasOf, TerrainResources } from "../view/terrain/gpu/resources";
import {
  ContactRecords,
  InstanceRecords,
  parentQuadrant,
  patchTerms,
} from "../view/terrain/gpu/uniforms";
import { parentKey, type PatchKey } from "../view/terrain/patchKey";
import type { BodyFigure } from "../view/terrain/planet";
import { type TerrainLighting, terrainLitMaterialSpec } from "../view/terrain/terrainMaterial";
import type {
  BakedPatch,
  BakeSettings,
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../view/terrain/workers/messages";
import { moonLikeFixture, type WorkerFixture } from "../test/moonLikeFixture";
import { type Checks, halfTexels, projection } from "./harness";

/** The Moon-like world's equatorial radius, m: R ÷ ∛(1 − f) ({@link MOON_LIKE}). */
const MOON_LIKE_A_M = 1.7374e6 / Math.cbrt(1 - 0.0012);

/**
 * The Moon-like world's figure (`hyperion_surface`'s `testing/worlds.rs`: the Moon's volumetric
 * radius, 1,737.4 km, at a flattening of 0.0012, `Spheroid::from_volumetric`'s a = R ÷ ∛(1 − f)).
 */
export const MOON_LIKE: Readonly<Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">> = {
  equatorialRadiusM: MOON_LIKE_A_M,
  polarRadiusM: MOON_LIKE_A_M * (1 - 0.0012),
};

/** The frames' size, pixels. */
const FRAME = { widthPx: 64, heightPx: 48 } as const;

/** The frames' cache budget, 32 MiB: a few hundred lit slots on either path. */
const CACHE_BYTES = 2 ** 25;

/** How long the worker may take over the field and the bakes, milliseconds. */
const WORKER_TIMEOUT_MS = 120_000;

/** The block's centre patch: level 7 over the first survey's cells (22, 22) to (23, 23) of face +y. */
const CENTRE: PatchKey = { face: 1, level: 7, i: 45, j: 45 };

/** The camera's height above the centre patch's origin, metres, and its depression, degrees. */
const HEIGHT_M = 30_000;
const DEPRESSION_DEG = 60;

/** The 3 × 3 block of keys about {@link CENTRE}, the centre first. */
function keys(): PatchKey[] {
  const out: PatchKey[] = [CENTRE];
  for (let dj = -1; dj <= 1; dj += 1) {
    for (let di = -1; di <= 1; di += 1) {
      if (di !== 0 || dj !== 0) {
        out.push({ ...CENTRE, i: CENTRE.i + di, j: CENTRE.j + dj });
      }
    }
  }
  return out;
}

/** The parents of `children`, each once, in the order first met. */
function parentsOf(children: ReadonlyArray<PatchKey>): PatchKey[] {
  const out: PatchKey[] = [];
  for (const child of children) {
    const parent = parentKey(child);
    if (parent !== null && !out.some((p) => p.i === parent.i && p.j === parent.j)) {
      out.push(parent);
    }
  }
  return out;
}

/**
 * The morph's start and end distances the block is drawn with, metres: the block lies 30 to about
 * 60 km from the camera, so that its morph factor is about ¼ to ½ and the horizon is blended to the
 * parents' maps (R10.T8.b).
 */
const MORPH_M = [0, 120_000] as const;

/** Posts the fixture's field to a fresh height worker and bakes `wanted` over it. */
async function bakeOverField(
  wanted: ReadonlyArray<PatchKey>,
  settings: BakeSettings,
  field: WorkerFixture,
): Promise<BakedPatch[]> {
  const worker = new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
    type: "module",
  });
  try {
    return await new Promise<BakedPatch[]>((resolve, reject) => {
      const baked = new Map<number, BakedPatch>();
      let held = 0;
      const timer = setTimeout(() => {
        reject(new Error(`${String(baked.size)} of ${String(wanted.length)} bakes in time`));
      }, WORKER_TIMEOUT_MS);
      const fail = (message: string): void => {
        clearTimeout(timer);
        reject(new Error(message));
      };
      worker.addEventListener("message", (event: MessageEvent<HeightWorkerReply>) => {
        const reply = event.data;
        switch (reply.kind) {
          case "field-held":
            held += 1;
            if (held === field.blocks.length) {
              wanted.forEach((key, id) => {
                const request: HeightWorkerRequest = {
                  kind: "bake",
                  id,
                  key,
                  generation: 1,
                  settings,
                };
                worker.postMessage(request, []);
              });
            }
            return;
          case "baked":
            baked.set(reply.id, reply.bake);
            if (baked.size === wanted.length) {
              clearTimeout(timer);
              resolve(wanted.map((_, id) => baked.get(id)).filter((b) => b !== undefined));
            }
            return;
          case "bake-failed":
          case "bake-unsurveyed":
          case "field-refused":
          // No class map is asked here (R10.T10.d).
          case "class-map":
          case "class-map-failed":
          // No readout is asked here (R10.T12).
          case "ground-readout":
          case "ground-readout-failed":
            fail(`the worker answered ${JSON.stringify(reply)}`);
            return;
        }
      });
      worker.addEventListener("error", (event: Event) => {
        fail(event instanceof ErrorEvent ? event.message : "the worker did not load");
      });
      field.blocks.forEach((block, n) => {
        const request: HeightWorkerRequest =
          n === 0
            ? { kind: "field-init", id: n, body: field.body, block, form: "payload" }
            : { kind: "field-chunk", id: n, body: field.body, block };
        worker.postMessage(request, []);
      });
    });
  } finally {
    worker.terminate();
  }
}

interface Xyz {
  readonly x: number;
  readonly y: number;
  readonly z: number;
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

/** What one frame of the check found. */
interface LitFrame {
  readonly pass: boolean;
  readonly detail: string;
}

/**
 * Draws the field's block with `setting`'s path and reads it back, releasing what it made whether
 * or not a step throws.
 */
async function drawLit(
  engine: RenderEngine,
  setting: "high" | "low",
  field: WorkerFixture,
): Promise<LitFrame> {
  const settings = { ...TERRAIN_SETTINGS[setting], cacheBytes: CACHE_BYTES };
  const resources = new TerrainResources(engine, settings, MOON_LIKE, "lit");
  const air = createAirlessTables(engine, "lit terrain smoke: no air");
  try {
    return await drawInto(engine, setting, field, resources, air);
  } finally {
    resources.dispose();
    engine.releaseTexture(air.body);
  }
}

/** {@link drawLit}'s frame, into `resources` lit through `air`. */
async function drawInto(
  engine: RenderEngine,
  setting: "high" | "low",
  field: WorkerFixture,
  resources: TerrainResources,
  air: SurfaceLightingTables,
): Promise<LitFrame> {
  const settings = TERRAIN_SETTINGS[setting];
  const { layout } = resources;
  const children = keys();
  const parents = parentsOf(children);
  const baked = await bakeOverField(
    [...children, ...parents],
    {
      vertexPath: layout.vertexPath,
      normals: settings.normals,
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
    },
    field,
  );
  baked.forEach((bake, slot) => {
    const result = resources.upload({ slot, ...bake });
    if (result.kind !== "uploaded") {
      throw new Error(`slot ${String(slot)} refused: ${result.reason}`);
    }
    if (!resources.holdsHorizon(slot)) {
      throw new Error(`slot ${String(slot)} holds no horizon map`);
    }
  });
  // The block alone is drawn; its parents' maps, in the slots after it, are what it blends to.
  const bakes = baked.slice(0, children.length);
  const centre = bakes[0];
  if (centre === undefined) {
    throw new Error("no centre patch was baked");
  }
  // The camera above the centre's origin along its datum normal; body-fixed axes are the frame's.
  const nu = patchTerms(centre.key, MOON_LIKE, centre.originHeightM).nu0;
  const camera = along(centre.originM, 1, nu, HEIGHT_M);
  const reference = Math.abs(nu.z) > 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 0, z: 1 };
  const east = unit(cross(reference, nu));
  const north = cross(nu, east);
  const d = (DEPRESSION_DEG * Math.PI) / 180;
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
    const parent = parentKey(bake.key);
    const index = parents.findIndex((p) => p.i === parent?.i && p.j === parent.j);
    if (index < 0) {
      throw new Error(`no parent of slot ${String(slot)} was baked`);
    }
    const parentSlot = children.length + index;
    instances.push(
      slot,
      { x: bake.originM.x - camera.x, y: bake.originM.y - camera.y, z: bake.originM.z - camera.z },
      MORPH_M[0],
      MORPH_M[1],
      parentSlot,
      parentQuadrant(bake.key),
    );
  });
  resources.writeFrame(instances, new ContactRecords());
  // A Sun-like star 40° from the zenith towards the east, at 1 au (IAU 2012 B2's 1.495978707 ×
  // 10¹¹ m) with the Sun's nominal radius (IAU 2015 B3's 6.957 × 10⁸ m) and about the Sun's
  // illuminance at 1 au, and an Earth-like neighbour's planetshine from the west, low, at the Moon's
  // distance and the Earth's radius over the Moon-like a (3.844 × 10⁸ m and 6.371 × 10⁶ m), its
  // illuminance a test value; the lights are relative to the centre ÷ a, body-fixed.
  const a = MOON_LIKE.equatorialRadiusM;
  const annuli = annulusEdges(0.6, 0.2, 2);
  const lighting: TerrainLighting = {
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
    secondaries: [
      {
        direction: unit(along(nu, 0.2, east, -1)),
        distance: 3.844e8 / a,
        radius: 6.371e6 / a,
        illuminance: [0.05, 0.05, 0.06],
      },
    ],
    elsewhere: withTemplateCurve({
      a: [0.11, 0.11, 0.11],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    }),
    exposureScale: 1e-4,
  };
  resources.setLighting(lighting);
  const lit = resources.lit;
  const classLayers = resources.field("class-layers");
  const horizonMap = resources.field("horizon-map");
  const surveyMask = resources.field("survey-mask");
  if (lit === null || classLayers === null || horizonMap === null || surveyMask === null) {
    throw new Error("the lit layout has no lit resources");
  }
  const storageBuffers: Record<string, BufferHandle> = {
    slots: resources.slotRecords,
    instances: resources.instances,
    contacts: resources.contacts,
    classLayers,
    terrainLights: lit.lights,
    horizonMap,
    surveyMask,
  };
  for (const name of ["heights", "offsets"] as const) {
    const buffer = resources.field(name);
    if (buffer !== null) {
      storageBuffers[name] = buffer;
    }
  }
  const material = await engine.createMaterialAsync(
    terrainLitMaterialSpec(layout.vertexPath),
    ["rgba16float"],
    [resources.mesh],
  );
  const target = engine.createRenderTarget({
    name: `lit terrain ${setting}`,
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  try {
    return await renderInto(engine, target, {
      resources,
      material,
      air,
      lit,
      storageBuffers,
      orientation,
      camera,
      inverseRadius: 1 / a,
      patches: bakes.length,
    });
  } finally {
    target.dispose();
  }
}

/** One frame's draw, as {@link drawInto} assembles it. */
interface LitDraw {
  readonly resources: TerrainResources;
  readonly material: MaterialHandle;
  readonly air: SurfaceLightingTables;
  readonly lit: { readonly lights: BufferHandle; readonly phaseTable: TextureHandle };
  readonly storageBuffers: Readonly<Record<string, BufferHandle>>;
  readonly orientation: Quaternion;
  readonly camera: Xyz;
  /** 1 ÷ a, per metre. */
  readonly inverseRadius: number;
  readonly patches: number;
}

/** Renders `draw` into `target` and reads it back. */
async function renderInto(
  engine: RenderEngine,
  target: RenderTarget,
  draw: LitDraw,
): Promise<LitFrame> {
  const { resources, material, air, lit, storageBuffers, orientation, camera } = draw;
  const { layout } = resources;
  const atlas = normalsAtlasOf(layout);
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
          cameraBodyFixedM: new Float32Array([camera.x, camera.y, camera.z, draw.inverseRadius]),
          atlas: new Float32Array([
            atlas.columns,
            atlas.tilesPerLayer,
            atlas.tileTexels,
            atlas.samplesPerSide,
          ]),
        },
        textures: {
          normals: resources.normals,
          phaseFactorTable: lit.phaseTable,
          ...surfaceLightingDrawTextures(air),
        },
        storageBuffers,
        indirect: { buffer: resources.indirect, offsetBytes: 0 },
      },
    ],
    postProcesses: [],
  });
  const texels = halfTexels(await engine.readTexture(target.colour));
  let terrain = 0;
  let litPixels = 0;
  for (let p = 0; p < FRAME.widthPx * FRAME.heightPx; p += 1) {
    // The meter class: 2 lit by a star, 3 unlit (R07's `METER_CLASS`).
    const meter = texels[4 * p + 3];
    if (meter === 2 || meter === 3) {
      terrain += 1;
    }
    if (meter === 2 && (texels[4 * p + 1] ?? 0) > 0) {
      litPixels += 1;
    }
  }
  const cover = terrain / (FRAME.widthPx * FRAME.heightPx);
  const finite = texels.every(Number.isFinite);
  return {
    pass: finite && cover >= 0.5 && litPixels > 0,
    detail: `${layout.vertexPath}: ${String(draw.patches)} patches, ${String(resources.phaseRows.laws.length)} phase rows, terrain ${(100 * cover).toFixed(1)}% of the frame, ${String(litPixels)} lit pixels, finite ${String(finite)}`,
  };
}

/** R10.T10.b's lit terrain: a generated world's patch set by both vertex paths. */
export async function checkLitTerrain(engine: RenderEngine, checks: Checks): Promise<void> {
  const field = moonLikeFixture("first");
  for (const setting of ["high", "low"] as const) {
    // The frames run in order: each bakes, draws and reads back before the next allocates.
    // oxlint-disable-next-line no-await-in-loop
    const { pass, detail } = await drawLit(engine, setting, field);
    checks.check(
      `R10.T10.b the lit terrain of a generated world (${setting}) is finite, drawn and lit`,
      pass,
      detail,
    );
  }
}
