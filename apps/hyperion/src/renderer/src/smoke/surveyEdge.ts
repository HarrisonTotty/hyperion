/**
 * The smoke harness's survey-edge check (plan R10, R10.T10.c; Design note 12): a half-surveyed
 * patch set of a generated world drawn by R10's lit material over R07's reference surface, the
 * body's mesh figure drawn beneath with the inverse of the GPU's coverage mask, on the real device.
 *
 * @remarks
 * The world is the Moon-like fixture's first survey (`smoke/litTerrain.ts`' field), four cells of
 * level 6 on face +y, taken by a height worker; the coverage mask follows the worker's blocks
 * (`CoverageMask.survey`). A 4 × 4 block of level-7 patches is baked across the survey's +i edge,
 * its two columns of lower i surveyed and its two of higher i not, and seen from 30 km above the
 * survey's side, about 60° below the horizon, towards the edge, under one Sun-like star (the
 * camera's pitch puts the edge between two rows of pixels, `poseOver`). Two frames:
 * - with the bakes' heights, the reference surface then the terrain into one scene target: every
 *   texel finite and every pixel drawn by one or the other, so that the skirts close the step
 *   between the terrain, some hundreds of metres below the datum there, and the reference surface;
 * - with the same masks over the datum (every height 0, so that the two surfaces coincide and no
 *   relief stands one in front of the other), each surface alone into its own target: no pixel
 *   drawn by both, and each drawing its half.
 *
 * Then the same block on the limb (R10.T10.c's deferred finding, "the limb draw keeps no mask"),
 * seen along the datum's tangent plane at the survey's edge, so that the limb runs across the view
 * with the survey's last surveyed cell on its left and the first unsurveyed one on its right: the
 * body's limb, the disc's second draw for a mesh body, drawn alone, draws no pixel over the
 * surveyed cell, whose ground is the terrain's, and draws the limb over the other.
 */

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { withTemplateCurve } from "../view/appearance/phase";
import {
  createAirlessTables,
  surfaceLightingDrawTextures,
  type SurfaceLightingTables,
} from "../view/atmosphere/surfaceLighting";
import { viewRay } from "../view/bodies/discShading";
import { classMapTexelOf } from "../view/bodies/discSurface";
import {
  type BodyFrameOptions,
  type BodyFramePlan,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../view/bodies/draw";
import { sphereFootprint } from "../view/bodies/regime";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  project,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../view/camera/projection";
import { lookAlong, rotate } from "../view/camera/quaternion";
import { IDENTITY_ROTATION } from "../view/coords/rotation";
import type {
  BufferHandle,
  DrawItem,
  MaterialHandle,
  RenderEngine,
  TextureHandle,
} from "../view/engine/types";
import { annulusEdges, DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { METER_CLASS } from "../view/post/meter";
import { TERRAIN_SETTINGS, type TerrainSettings } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import { CoverageMask, fieldCellAt } from "../view/terrain/coverage";
import { handoverTauPx, limbDistanceM, pixelAngleRad } from "../view/terrain/handover";
import { normalsAtlasOf, TerrainResources } from "../view/terrain/gpu/resources";
import { vertexDir } from "../view/terrain/cube";
import { ContactRecords, InstanceRecords } from "../view/terrain/gpu/uniforms";
import { SELECTION_RULE, sigmaSurveyOf } from "../view/terrain/levelBounds";
import type { PatchKey } from "../view/terrain/patchKey";
import { spheroidNormal, surfacePoint } from "../view/terrain/planet";
import { createShadowPlaceholder } from "../view/terrain/shadows/terrainShadows";
import {
  SHADOW_CASCADES_TEXTURE,
  type TerrainLighting,
  terrainLitMaterialSpec,
} from "../view/terrain/terrainMaterial";
import type {
  BakedPatch,
  BakeSettings,
  FieldBlockAnswer,
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../view/terrain/workers/messages";
import { WIREFRAME_MATERIALS } from "../view/wireframe/submit";
import type { CapturedImage } from "./atmosphere";
import { type Checks, halfTexels } from "./harness";
import { hdrCapture } from "./hdrCapture";
import { moonLikeFixture, type WorkerFixture } from "../test/moonLikeFixture";
import { MOON_LIKE } from "./litTerrain";

/** The frames' size, pixels (R10.T16.c's frame check takes it too). */
export const VIEWPORT: Viewport = { widthPx: 64, heightPx: 48 };

/** The frames' cache budget, 32 MiB: a few hundred lit slots. */
const CACHE_BYTES = 2 ** 25;

/** How long the worker may take over the field and the bakes, milliseconds. */
const WORKER_TIMEOUT_MS = 120_000;

/** The pre-exposure the frames are drawn at. */
export const EXPOSURE = 1e-4;

/**
 * The block: level 7 of face +y, i 46 to 49 and j 44 to 47. Columns 46 and 47 lie in the survey's
 * cell 23, 48 and 49 in cell 24, which it does not survey; rows 44 to 47 in its cells 22 and 23.
 */
function blockKeys(): PatchKey[] {
  const keys: PatchKey[] = [];
  for (let j = 44; j <= 47; j += 1) {
    for (let i = 46; i <= 49; i += 1) {
      keys.push({ face: 1, level: 7, i, j });
    }
  }
  return keys;
}

/** The patch the camera stands over: the surveyed column next to the edge. */
export const OVER: PatchKey = { face: 1, level: 7, i: 47, j: 46 };

/** The camera's height above that patch's origin, metres, and its depression, degrees. */
const HEIGHT_M = 30_000;
const DEPRESSION_DEG = 60;

/** The field's blocks' answers and the bakes, from a fresh height worker. */
async function bakeAcrossTheEdge(
  wanted: ReadonlyArray<PatchKey>,
  settings: BakeSettings,
  field: WorkerFixture,
): Promise<{ readonly bakes: BakedPatch[]; readonly blocks: FieldBlockAnswer[] }> {
  const worker = new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
    type: "module",
  });
  try {
    return await new Promise((resolve, reject) => {
      const baked = new Map<number, BakedPatch>();
      const blocks: FieldBlockAnswer[] = [];
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
            if (reply.block !== null) {
              blocks.push(reply.block);
            }
            if (blocks.length === field.blocks.length) {
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
              resolve({
                bakes: wanted.map((_, id) => baked.get(id)).filter((b) => b !== undefined),
                blocks,
              });
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
          // No bake is drafted here, so none is refined (R10.T6.a).
          case "normals-refined":
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
            ? {
                kind: "field-init",
                id: n,
                body: field.body,
                block,
                form: "payload",
                sigma: sigmaSurveyOf(SELECTION_RULE),
              }
            : { kind: "field-chunk", id: n, body: field.body, block };
        worker.postMessage(request, []);
      });
    });
  } finally {
    worker.terminate();
  }
}

/** The block baked: its setting, its bakes and the field's blocks the worker took. */
interface BakedBlock {
  readonly settings: TerrainSettings;
  readonly bakes: ReadonlyArray<BakedPatch>;
  readonly blocks: ReadonlyArray<FieldBlockAnswer>;
}

/** The block's bakes on the low setting from a fresh worker, and the field's blocks it took. */
async function bakeTheBlock(): Promise<BakedBlock> {
  const field = moonLikeFixture("first");
  const settings = { ...TERRAIN_SETTINGS.low, cacheBytes: CACHE_BYTES };
  const { bakes, blocks } = await bakeAcrossTheEdge(
    blockKeys(),
    {
      vertexPath: settings.vertexPath,
      normals: settings.normals,
      source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
    },
    field,
  );
  return { settings, bakes, blocks };
}

/** A bake with every height on the datum: its own and morph heights and its origin's, 0. */
export function onTheDatum(bake: BakedPatch): BakedPatch {
  return { ...bake, heights: new Float32Array(bake.heights.length), originHeightM: 0 };
}

/** The camera, body-fixed, which the frame's axes are: its place, orientation and projection. */
export interface Pose {
  /** From the body's centre, m. */
  readonly cameraM: Vec3;
  readonly camera: ProjectionCamera;
  /** The unit direction of the star from the body's centre. */
  readonly sun: Vec3;
}

/** The datum's point over vertex (x, y) of `key`, body-fixed m, and its normal there. */
export function datumAt(
  key: PatchKey,
  x: number,
  y: number,
): { readonly p: Vec3; readonly nu: Vec3 } {
  const d = vertexDir(key, x, y);
  const [px, py, pz] = surfacePoint({ ...MOON_LIKE, pole: null }, d, 0);
  const [nx, ny, nz] = spheroidNormal({ ...MOON_LIKE, pole: null }, d);
  return { p: vec3(px, py, pz), nu: vec3(nx, ny, nz) };
}

/**
 * 30 km over the midpoint of {@link OVER}'s lower edge (i 47.5, j 46 of level 7), looking towards
 * +i about 60° below the horizon, the Sun 40° from the zenith behind the camera.
 *
 * @remarks
 * The camera is pitched, from 60°, so that the survey's edge on the datum ahead, at i 48, falls on
 * the edge between two rows of pixels at the view's centre column: the reference surface finds a
 * pixel's cell from its ray's hit on the analytic spheroid in `f32`, a few tenths of a metre from
 * the terrain's own, exact in its patch's coordinates, so that a pixel's centre within that of the
 * edge could be drawn by both or by neither. The edge's curve on the view stays within about 0.3 px
 * of that row's edge across the view, and every pixel's centre a kilometre-scale distance from it.
 * The survey's edges along i, at j 44 and 48, lie outside the view on the ground nearer than i 48.
 */
export function poseOver(): Pose {
  const { p, nu } = datumAt(OVER, 32, 0);
  const cameraM = add(p, scale(nu, HEIGHT_M));
  // Face +y's u axis, along which i rises, is −x (`faceUvToXyz`): east here, made level.
  const along = vec3(-1, 0, 0);
  const level = add(along, scale(nu, -(along.x * nu.x + along.y * nu.y + along.z * nu.z)));
  const east = scale(level, 1 / Math.hypot(level.x, level.y, level.z));
  const sun = add(scale(nu, Math.cos(0.7)), scale(east, -Math.sin(0.7)));
  const edge = datumAt(OVER, 64, 0).p;
  const cameraAt = (depressionRad: number): ProjectionCamera => ({
    orientation: lookAlong(
      add(scale(east, Math.cos(depressionRad)), scale(nu, -Math.sin(depressionRad))),
      nu,
    ),
    fovXRad: Math.PI / 2,
  });
  const edgeRowPx = (depressionRad: number): number =>
    project(
      vec3(edge.x - cameraM.x, edge.y - cameraM.y, edge.z - cameraM.z),
      cameraAt(depressionRad),
      VIEWPORT,
    ).yPx;
  // A secant search for the pitch that puts the edge on the nearest row's edge.
  let a = (DEPRESSION_DEG * Math.PI) / 180;
  const target = Math.round(edgeRowPx(a));
  let b = a + 1e-3;
  for (let step = 0; step < 8; step += 1) {
    const fa = edgeRowPx(a) - target;
    const fb = edgeRowPx(b) - target;
    if (fb === fa) {
      break;
    }
    const next = b - (fb * (b - a)) / (fb - fa);
    a = b;
    b = next;
  }
  return {
    cameraM,
    camera: cameraAt(b),
    sun: scale(sun, 1 / Math.hypot(sun.x, sun.y, sun.z)),
  };
}

/** The limb's camera: its distance from the survey's edge along the datum's tangent plane, m. */
const LIMB_RANGE_M = 200_000;

/** The limb's camera's field of view across, rad: 6°, about 330 m a pixel at the limb. */
const LIMB_FOV_RAD = (6 * Math.PI) / 180;

/**
 * The survey's +i edge on the limb: the camera {@link LIMB_RANGE_M} from the edge's point at i 48,
 * j 46.5 of level 7 along the datum's tangent plane there towards −j, looking back along it with
 * the datum's normal up, pitched down by half a pixel so that the limb runs along the middle of the
 * row above the view's centre (straight to a tenth of a pixel), its left half over the survey's
 * cell 23 and its right half over cell 24, which it does not survey, the edge between two columns;
 * the Sun 45° up behind the camera. The ground a pixel inside the limb, which its rays meet, lies
 * some 33 km from the edge's point towards the camera, within the survey's rows (j 44 to 48).
 */
export function poseAtTheLimb(): Pose {
  const edge: PatchKey = { face: 1, level: 7, i: 48, j: 46 };
  const { p, nu } = datumAt(edge, 0, 32);
  const towardsLowerJ = sub(datumAt(edge, 0, 0).p, p);
  const along = normalise(sub(towardsLowerJ, scale(nu, dot(towardsLowerJ, nu))));
  const halfPixelRad = Math.tan(LIMB_FOV_RAD / 2) / VIEWPORT.widthPx;
  const sun = normalise(add(nu, along));
  return {
    cameraM: add(p, scale(along, LIMB_RANGE_M)),
    camera: {
      orientation: lookAlong(add(scale(along, -1), scale(nu, -halfPixelRad)), nu),
      fovXRad: LIMB_FOV_RAD,
    },
    sun,
  };
}

/** A pixel's ray in the spheroid's scaled space, body-fixed, the pole stretched by a ÷ c. */
interface ScaledRay {
  /** The ray's unit direction from the camera there. */
  readonly ray: Vec3;
  /** The body's centre from the camera there, m. */
  readonly centreM: Vec3;
  /** The sphere's radius there, a, m. */
  readonly radiusM: number;
}

/** The ray through (`xPx`, `yPx`) from `pose`, in the spheroid's scaled space. */
function scaledRay(pose: Pose, xPx: number, yPx: number): ScaledRay {
  const stretch = MOON_LIKE.equatorialRadiusM / MOON_LIKE.polarRadiusM;
  const scaled = (v: Vec3): Vec3 => vec3(v.x, v.y, v.z * stretch);
  const ray = rotate(pose.camera.orientation, viewRay(xPx, yPx, pose.camera, VIEWPORT));
  return {
    ray: normalise(scaled(ray)),
    centreM: scaled(scale(pose.cameraM, -1)),
    radiusM: MOON_LIKE.equatorialRadiusM,
  };
}

/**
 * The signed angle of the ray through (`xPx`, `yPx`) from the spheroid's limb, rad, positive
 * outside: `bodyDisc.wgsl`'s `limb_angle` in `f64`.
 */
function limbAngleAt(pose: Pose, xPx: number, yPx: number): number {
  const { ray, centreM, radiusM } = scaledRay(pose, xPx, yPx);
  return (
    Math.atan2(norm(cross(ray, centreM)), dot(ray, centreM)) - Math.asin(radiusM / norm(centreM))
  );
}

/**
 * The coarse cell of `level` beneath the ray through (`xPx`, `yPx`), as a mesh body's limb draw
 * finds it (`bodyDiscDraw.wgsl`'s `limb_surveyed`), in `f64`: the ray's hit on the spheroid, or
 * where it passes outside, the spheroid's point nearest it.
 */
function limbCellAt(pose: Pose, xPx: number, yPx: number, level: number): string {
  const { ray, centreM, radiusM } = scaledRay(pose, xPx, yPx);
  const offset = sub(scale(ray, dot(ray, centreM)), centreM);
  const h2 = radiusM * radiusM - dot(offset, offset);
  const ground = h2 >= 0 ? sub(offset, scale(ray, Math.sqrt(h2))) : offset;
  const { face, i, j } = classMapTexelOf([ground.x, ground.y, ground.z], 2 ** level);
  return `${String(face)}/${String(i)}/${String(j)}`;
}

/** The cells `blocks` surveyed, as {@link limbCellAt} names them. */
function surveyedCells(blocks: ReadonlyArray<FieldBlockAnswer>): ReadonlySet<string> {
  const cells = new Set<string>();
  for (const { fieldLevel, newlySurveyed: runs } of blocks) {
    for (let r = 0; r + 1 < runs.length; r += 2) {
      for (let index = runs[r] ?? 0; index < (runs[r + 1] ?? 0); index += 1) {
        const { face, i, j } = fieldCellAt(fieldLevel, index);
        cells.add(`${String(face)}/${String(i)}/${String(j)}`);
      }
    }
  }
  return cells;
}

/**
 * The terrain's draw of `resources` from `pose`, lit by its star through a body without air, with
 * no shadow cascades: it binds their placeholder, `shadow` (R10.T9.b).
 */
async function terrainDraw(
  engine: RenderEngine,
  resources: TerrainResources,
  pose: Pose,
  air: SurfaceLightingTables,
  shadow: TextureHandle,
): Promise<DrawItem> {
  const { layout } = resources;
  const a = MOON_LIKE.equatorialRadiusM;
  const annuli = annulusEdges(0.6, 0.2, 2);
  const lighting: TerrainLighting = {
    lights: [
      {
        direction: pose.sun,
        distance: AU_M / a,
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
    exposureScale: EXPOSURE,
  };
  resources.setLighting(lighting);
  const lit = resources.lit;
  const buffers: Record<string, BufferHandle | null> = {
    heights: resources.field("heights"),
    slots: resources.slotRecords,
    instances: resources.instances,
    contacts: resources.contacts,
    classLayers: resources.field("class-layers"),
    terrainLights: lit?.lights ?? null,
    horizonMap: resources.field("horizon-map"),
    surveyMask: resources.field("survey-mask"),
  };
  const storageBuffers: Record<string, BufferHandle> = {};
  for (const [name, buffer] of Object.entries(buffers)) {
    if (buffer === null) {
      throw new Error(`the lit layout has no ${name}`);
    }
    storageBuffers[name] = buffer;
  }
  if (lit === null) {
    throw new Error("the lit layout has no lights");
  }
  const material: MaterialHandle = await engine.createMaterialAsync(
    terrainLitMaterialSpec(layout.vertexPath),
    ["rgba16float"],
    [resources.mesh],
  );
  const atlas = normalsAtlasOf(layout);
  const c = pose.cameraM;
  return {
    mesh: resources.mesh,
    material,
    offsetFromCameraM: new Float32Array(3),
    uniforms: {
      bodyRotation: new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
      cameraBodyFixedM: new Float32Array([c.x, c.y, c.z, 1 / a]),
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
      [SHADOW_CASCADES_TEXTURE]: shadow,
    },
    storageBuffers,
    indirect: { buffer: resources.indirect, offsetBytes: 0 },
  };
}

/** Uploads `bakes` into `resources`' first slots and writes their instances from `pose`. */
function place(resources: TerrainResources, bakes: ReadonlyArray<BakedPatch>, pose: Pose): void {
  const instances = new InstanceRecords(resources.layout.slots.slotCount);
  bakes.forEach((bake, slot) => {
    const result = resources.upload({ slot, ...bake });
    if (result.kind !== "uploaded") {
      throw new Error(`slot ${String(slot)} refused: ${result.reason}`);
    }
    const o = bake.originM;
    const c = pose.cameraM;
    instances.push(slot, { x: o.x - c.x, y: o.y - c.y, z: o.z - c.z }, 0, 0);
  });
  resources.writeFrame(instances, new ContactRecords());
}

/** The reference surface's draws: the body as a mesh over its terrain, beneath its survey. */
function referenceDraws(
  renderer: LitBodyRenderer,
  pose: Pose,
  mask: CoverageMask,
): ReadonlyArray<DrawItem> {
  return renderer.meshDraws(referencePlan(pose, mask));
}

/** The reference surface's plan: the body handed over to its terrain and drawn as a mesh. */
function referencePlan(pose: Pose, mask: CoverageMask): BodyFramePlan {
  const texture = mask.texture;
  if (texture === null) {
    throw new Error("the coverage mask holds nothing");
  }
  const centreM = scale(pose.cameraM, -1);
  const figure = { ...MOON_LIKE, pole: vec3(0, 0, 1) };
  // Handed over to its terrain (R10.T10.e): a relief of four times the setting's τ at its limb.
  const limbM = limbDistanceM(norm(centreM), figure.equatorialRadiusM);
  const body: LitBodyInput = {
    id: "5e5e000000000000.0003",
    centreM,
    figure,
    photometry: PROVISIONAL_PHOTOMETRY,
    rotation: IDENTITY_ROTATION,
    surveyCoverage: { mask: texture, rotation: IDENTITY_ROTATION },
    handoverReliefM:
      4 * handoverTauPx("photorealistic", "low") * limbM * pixelAngleRad(pose.camera, VIEWPORT),
    lighting: undefined,
  };
  // The terrain writes depth over the whole view, and the body is drawn as a mesh beneath it.
  const writer = sphereFootprint(centreM, figure.equatorialRadiusM, pose.camera, VIEWPORT);
  const options: BodyFrameOptions = {
    camera: pose.camera,
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    exposureScale: EXPOSURE,
    annuli: DISC_ANNULI_HIGH,
    planetshine: PLANETSHINE_SOURCES_HIGH,
    setting: "low",
    depthWriters: writer === null ? [] : [writer],
  };
  const plan = planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(pose.sun, AU_M)) }],
    options,
    new Map([[body.id, "disc"]]),
  );
  if (plan.meshes.length !== 1 || plan.meshes[0]?.role.kind !== "handed-over") {
    throw new Error("the body was not handed over to its terrain and drawn as a mesh");
  }
  return plan;
}

/** Draws `passes` in order into a fresh scene target from `pose` and reads it back. */
export async function drawn(
  engine: RenderEngine,
  pose: Pose,
  passes: ReadonlyArray<{ readonly label: string; readonly draws: ReadonlyArray<DrawItem> }>,
): Promise<Float32Array> {
  const target = createSceneTarget(engine, "smoke survey edge", VIEWPORT);
  try {
    const viewRotation = viewRotation4(pose.camera.orientation);
    const projection = perspectiveReversedInfinite(
      pose.camera.fovXRad,
      VIEWPORT.widthPx / VIEWPORT.heightPx,
      NEAR_PLANE_M,
    );
    passes.forEach(({ label, draws }, n) => {
      target.render({
        label,
        viewRotation,
        projection,
        draws,
        postProcesses: [],
        ...(n === 0 ? {} : { colourLoad: "load" as const }),
      });
    });
    return halfTexels(await engine.readTexture(target.colour));
  } finally {
    target.dispose();
  }
}

/** Whether pixel `p` of a read-back was drawn by a body: its meter class lit or unlit. */
export function bodyAt(texels: Float32Array, p: number): boolean {
  const meter = texels[4 * p + 3];
  return meter === METER_CLASS.litBody || meter === METER_CLASS.unlitBody;
}

/** R10.T10.c's survey edge: the terrain and the reference surface share the ground, once each. */
export async function checkSurveyEdge(engine: RenderEngine, checks: Checks): Promise<void> {
  const { settings, bakes, blocks } = await bakeTheBlock();
  const pose = poseOver();
  const relief = new TerrainResources(engine, settings, MOON_LIKE, "lit");
  const datum = new TerrainResources(engine, settings, MOON_LIKE, "lit");
  const air = createAirlessTables(engine, "survey edge smoke: no air");
  const shadow = createShadowPlaceholder(engine);
  const mask = new CoverageMask(engine, "survey edge smoke: coverage mask");
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  try {
    for (const block of blocks) {
      mask.survey(block);
    }
    place(relief, bakes, pose);
    place(datum, bakes.map(onTheDatum), pose);
    const reference = referenceDraws(renderer, pose, mask);
    const terrain = await terrainDraw(engine, relief, pose, air, shadow);
    const flat = await terrainDraw(engine, datum, pose, air, shadow);
    const bodies = PHOTOREAL_PASS_LABELS.bodies;
    const both = await drawn(engine, pose, [
      { label: bodies, draws: reference },
      { label: "terrain", draws: [terrain] },
    ]);
    const terrainAlone = await drawn(engine, pose, [{ label: "terrain", draws: [flat] }]);
    const referenceAlone = await drawn(engine, pose, [{ label: bodies, draws: reference }]);
    const pixels = VIEWPORT.widthPx * VIEWPORT.heightPx;
    let holes = 0;
    let doubles = 0;
    let terrainPixels = 0;
    let referencePixels = 0;
    for (let p = 0; p < pixels; p += 1) {
      holes += bodyAt(both, p) ? 0 : 1;
      const t = bodyAt(terrainAlone, p);
      const r = bodyAt(referenceAlone, p);
      terrainPixels += t ? 1 : 0;
      referencePixels += r ? 1 : 0;
      doubles += t && r ? 1 : 0;
    }
    const finite = both.every(Number.isFinite);
    const surveyed = blocks.reduce((n, b) => n + b.newlySurveyed.length / 2, 0);
    checks.check(
      "R10.T10.c a half-surveyed patch set and the reference surface beneath it draw every pixel, finite, and no pixel twice",
      finite &&
        holes === 0 &&
        doubles === 0 &&
        terrainPixels >= 0.1 * pixels &&
        referencePixels >= 0.1 * pixels,
      `${String(bakes.length)} patches over ${String(surveyed)} surveyed runs; with relief: finite ${String(finite)}, ${String(holes)} pixels undrawn; on the datum: terrain ${String(terrainPixels)}, reference ${String(referencePixels)}, both ${String(doubles)} of ${String(pixels)}`,
    );
    const limb = await limbFrames(engine, { renderer, mask, relief, bakes, air, shadow });
    checkLimb(checks, limb, blocks);
  } finally {
    renderer.dispose();
    mask.release();
    relief.dispose();
    datum.dispose();
    engine.releaseTexture(air.body);
    engine.releaseTexture(shadow);
  }
}

/** What the limb's frames draw with: the survey edge's renderer, mask, terrain and tables. */
interface LimbScene {
  readonly renderer: LitBodyRenderer;
  readonly mask: CoverageMask;
  /** The block's resources with its bakes' relief, placed again from the limb's pose. */
  readonly relief: TerrainResources;
  readonly bakes: ReadonlyArray<BakedPatch>;
  readonly air: SurfaceLightingTables;
  readonly shadow: TextureHandle;
}

/** The limb's two frames from {@link poseAtTheLimb}. */
interface LimbFrames {
  readonly pose: Pose;
  /** The body's limb draw alone, into a cleared target. */
  readonly alone: Float32Array;
  /**
   * The whole frame in the photorealistic style's order: the reference surface's mesh, the
   * terrain with its relief, then the limb.
   */
  readonly whole: Float32Array;
}

/** Draws the limb's frames of `scene`. */
async function limbFrames(engine: RenderEngine, scene: LimbScene): Promise<LimbFrames> {
  const pose = poseAtTheLimb();
  const plan = referencePlan(pose, scene.mask);
  place(scene.relief, scene.bakes, pose);
  const terrain = await terrainDraw(engine, scene.relief, pose, scene.air, scene.shadow);
  const limb = scene.renderer.draws(plan);
  const alone = await drawn(engine, pose, [{ label: PHOTOREAL_PASS_LABELS.discs, draws: limb }]);
  const whole = await drawn(engine, pose, [
    { label: PHOTOREAL_PASS_LABELS.bodies, draws: scene.renderer.meshDraws(plan) },
    { label: "terrain", draws: [terrain] },
    { label: PHOTOREAL_PASS_LABELS.discs, draws: limb },
  ]);
  return { pose, alone, whole };
}

/**
 * The limb's check (R10.T10.c's deferred finding): the limb draw alone draws no pixel whose ground
 * lies in a cell the survey holds, and draws the limb over the cells it does not. A pixel is on
 * the limb where its corners lie either side of it ({@link limbAngleAt}); its ground's cell is
 * {@link limbCellAt}'s at its centre, and a pixel whose centre lies within a quarter of a pixel of
 * another cell across the view is left out, since the shader finds the cell in `f32`. A pixel is drawn where its light is not 0: the
 * target is cleared to 0 and the Sun lights the limb's ground.
 */
function checkLimb(
  checks: Checks,
  frames: LimbFrames,
  blocks: ReadonlyArray<FieldBlockAnswer>,
): void {
  const { pose, alone } = frames;
  const level = blocks[0]?.fieldLevel ?? 0;
  const surveyed = surveyedCells(blocks);
  let onLimb = 0;
  let overSurvey = 0;
  let drawnOverSurvey = 0;
  let overBare = 0;
  let drawnOverBare = 0;
  for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
    for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
      const corners = [
        limbAngleAt(pose, x, y),
        limbAngleAt(pose, x + 1, y),
        limbAngleAt(pose, x, y + 1),
        limbAngleAt(pose, x + 1, y + 1),
      ];
      if (!(Math.min(...corners) < 0 && Math.max(...corners) > 0)) {
        continue;
      }
      onLimb += 1;
      const cell = limbCellAt(pose, x + 0.5, y + 0.5, level);
      if (
        limbCellAt(pose, x + 0.25, y + 0.5, level) !== cell ||
        limbCellAt(pose, x + 0.75, y + 0.5, level) !== cell
      ) {
        continue;
      }
      const at = 4 * (y * VIEWPORT.widthPx + x);
      const lit = (alone[at] ?? 0) > 0 || (alone[at + 1] ?? 0) > 0 || (alone[at + 2] ?? 0) > 0;
      if (surveyed.has(cell)) {
        overSurvey += 1;
        drawnOverSurvey += lit ? 1 : 0;
      } else {
        overBare += 1;
        drawnOverBare += lit ? 1 : 0;
      }
    }
  }
  const finite = alone.every(Number.isFinite) && frames.whole.every(Number.isFinite);
  checks.check(
    "R10.T10.c a mesh body's limb leaves its surveyed ground to the terrain and draws the rest",
    finite &&
      overSurvey >= 8 &&
      overBare >= 8 &&
      drawnOverSurvey === 0 &&
      drawnOverBare >= overBare - 2,
    `${String(onLimb)} pixels on the limb; over the survey ${String(drawnOverSurvey)} of ${String(overSurvey)} drawn; over bare ground ${String(drawnOverBare)} of ${String(overBare)} drawn; every texel finite ${String(finite)}`,
  );
}

/**
 * The limb's captures (R10.T10.c): the whole frame and the limb draw alone from
 * {@link poseAtTheLimb}, each pixel 4 × 4, at the frames' pre-exposure.
 */
export async function captureSurveyLimb(engine: RenderEngine): Promise<CapturedImage[]> {
  const { settings, bakes, blocks } = await bakeTheBlock();
  const relief = new TerrainResources(engine, settings, MOON_LIKE, "lit");
  const air = createAirlessTables(engine, "survey limb capture: no air");
  const shadow = createShadowPlaceholder(engine);
  const mask = new CoverageMask(engine, "survey limb capture: coverage mask");
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  try {
    for (const block of blocks) {
      mask.survey(block);
    }
    const frames = await limbFrames(engine, { renderer, mask, relief, bakes, air, shadow });
    return [
      hdrCapture("r10-t10c-limb-whole", frames.whole, VIEWPORT, 1, 4),
      hdrCapture("r10-t10c-limb-alone", frames.alone, VIEWPORT, 1, 4),
    ];
  } finally {
    renderer.dispose();
    mask.release();
    relief.dispose();
    engine.releaseTexture(air.body);
    engine.releaseTexture(shadow);
  }
}
