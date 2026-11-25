/**
 * The smoke harness's checks of plan R11's decorative scatter (R11.T6.a–b), test-render's
 * `scatter` part.
 *
 * @remarks
 * - **The cull against its twin** ({@link checkScatterCull}): `scatterCull.wgsl`'s
 *   `scatter_box_meets` over points about two boxes, a tilted craft's on the ground and one hovering
 *   0.1 m up, with spheres inside, straddling a face and beside it, read back against
 *   `intersectsBody`, bit for bit away from a face's rounding.
 * - **The pass over a generated world** ({@link checkPebbles}), on each vertex path: the Moon-like
 *   fixture's first survey baked by a real height worker into a 3 × 3 block of finest-level patches
 *   (their class weights, photometry and the rocks' abundances, `TerrainLayers.rockAbundances`),
 *   drawn by R10's lit material from 1.5 m above the block's centre, 45° below the horizon, under a
 *   Sun-like star with no air, at 256 × 192 px; its scatter placed by `ScatterPass` over the block
 *   with a body's box hovering 0.1 m up in view, culled and drawn in the same pass after the
 *   terrain. It checks that the pass places pebbles, every one below 0.4 m across and none meeting
 *   the box; that the GPU's pebbles are the CPU twin's (`placePebblesCpu`), their count to 1% and
 *   each matched by place, tint, patch, centre and radius; that half the density keeps about half
 *   as many, within the binomial interval; and that the frame with them is finite, its pebbles lit
 *   and drawn where the frame without them shows the ground.
 */

import { cross, normalise, type Vec3 } from "../geometry/vec3";
import {
  createAirlessTables,
  surfaceLightingDrawTextures,
} from "../view/atmosphere/surfaceLighting";
import { quaternionFromRows } from "../view/camera/quaternion";
import { viewRotation4 } from "../view/camera/projection";
import { cameraCullPassOf, viewAxisOf } from "../view/clusters/instanceScene";
import { PARTICLE_INSTANCE_BYTES } from "../view/clusters/particles";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, DrawItem, RenderEngine } from "../view/engine/types";
import { METER_CLASS } from "../view/post/meter";
import { TERRAIN_SETTINGS, type TerrainVertexPath } from "../view/quality/qualitySetting";
import { decorationSeed } from "../view/surface/decoration/decorationLattice";
import { PEBBLE_LARGEST_DIAMETER_M, pebbleLatticeLevels } from "../view/surface/scatter/pebbleLaw";
import {
  type PebbleTwinInstance,
  type PebbleTwinSlot,
  placePebblesCpu,
  type ScatterPatchRecord,
  scatterEntries,
} from "../view/surface/scatter/pebblePlace";
import {
  intersectsBody,
  MAX_SCATTER_BODIES,
  packScatterBoxes,
  SCATTER_BOX_BYTES,
  SCATTER_CULL_TWIN_KERNEL,
  type ScatterBox,
  scatterCullTwinWorkgroups,
} from "../view/surface/scatter/scatterCull";
import { ScatterKernelSet } from "../view/surface/scatter/scatterKernels";
import { ScatterPass } from "../view/surface/scatter/scatterPass";
import { finestLevel, PATCH_QUADS, vertexSpacing } from "../view/terrain/cube";
import { faceDifferencePositionF32, type SlotTermsF32 } from "../view/terrain/gpu/vertexEmulation";
import { normalsAtlasOf, TerrainResources } from "../view/terrain/gpu/resources";
import {
  ContactRecords,
  InstanceRecords,
  NO_PARENT_SLOT,
  parentQuadrant,
  patchTerms,
} from "../view/terrain/gpu/uniforms";
import type { PatchKey } from "../view/terrain/patchKey";
import { createShadowPlaceholder } from "../view/terrain/shadows/terrainShadows";
import {
  classPhotometryOf,
  DISC_NORMALS_TEXTURE,
  SHADOW_CASCADES_TEXTURE,
  TERRAIN_CLASSES,
  terrainLitMaterialSpec,
} from "../view/terrain/terrainMaterial";
import type { BakedPatch } from "../view/terrain/workers/messages";
import { withTemplateCurve } from "../view/appearance/phase";
import { annulusEdges } from "../view/lighting/annuli";
import { moonLikeFixture, type WorkerFixture } from "../test/moonLikeFixture";
import type { CapturedImage } from "./atmosphere";
import { type Checks, halfTexels, projection } from "./harness";
import { hdrCapture } from "./hdrCapture";
import { bakeOverField, MOON_LIKE } from "./litTerrain";

/** The frames' size, pixels: 4 : 3, wide enough that the near pebbles span many pixels. */
const FRAME = { widthPx: 256, heightPx: 192 } as const;

/** The frames' cache budget, 32 MiB. */
const CACHE_BYTES = 2 ** 25;

/** The camera's height above the block's centre, m, and its depression, degrees. */
const HEIGHT_M = 1.5;
const DEPRESSION_DEG = 45;

/** The share of the GPU's pebbles a twin must match, and the count's relative allowance. */
const MATCHED_SHARE = 0.99;
const COUNT_ALLOWANCE = 0.01;

/** How near a matched pebble's centre must lie, m, and its radius, relative. */
const CENTRE_ALLOWANCE_M = 0.005;
const RADIUS_ALLOWANCE = 1e-3;

/** The block's centre at the finest level: the descendant at the centre of lit terrain's (7, 45, 45). */
function blockKeys(): PatchKey[] {
  const level = finestLevel(MOON_LIKE.equatorialRadiusM);
  const scale = 2 ** (level - 7);
  const centre: PatchKey = { face: 1, level, i: 45 * scale + scale / 2, j: 45 * scale + scale / 2 };
  const out: PatchKey[] = [centre];
  for (let dj = -1; dj <= 1; dj += 1) {
    for (let di = -1; di <= 1; di += 1) {
      if (di !== 0 || dj !== 0) {
        out.push({ ...centre, i: centre.i + di, j: centre.j + dj });
      }
    }
  }
  return out;
}

/** `key`'s slot terms, narrowed as its slot record holds them (`test/strideOracle.ts`'). */
function termsOf(bake: BakedPatch): SlotTermsF32 {
  const terms = patchTerms(bake.key, MOON_LIKE, bake.originHeightM);
  const f = Math.fround;
  const v = (a: Vec3): Vec3 => ({ x: f(a.x), y: f(a.y), z: f(a.z) });
  const [a, e1, e2] = terms.axes;
  return {
    axisA: v(a),
    axisE1: v(e1),
    axisE2: v(e2),
    s0: f(terms.st0[0]),
    t0: f(terms.st0[1]),
    u0: f(terms.uv0[0]),
    v0: f(terms.uv0[1]),
    step: f(terms.step),
    scale: v(terms.scale),
    m0: v(terms.m0),
    nu0: v(terms.nu0),
    h0M: f(bake.originHeightM),
    straddles: terms.straddles,
  };
}

/** The twin's view of a slot's bake on `path`: its terms, its vertices and its class layers. */
function twinSlotOf(bake: BakedPatch, path: TerrainVertexPath): PebbleTwinSlot {
  const terms = termsOf(bake);
  const layers = bake.layers;
  if (layers === null) {
    throw new Error(`the bake of ${JSON.stringify(bake.key)} has no layers`);
  }
  const vertex = (x: number, y: number, which: 0 | 1): Vec3 => {
    const at = 65 * y + x;
    if (path === "baked-offsets") {
      const offsets = bake.offsets;
      if (offsets === null) {
        throw new Error("a baked-offsets bake has its offsets");
      }
      const i = 6 * at + 3 * which;
      return { x: offsets[i] ?? 0, y: offsets[i + 1] ?? 0, z: offsets[i + 2] ?? 0 };
    }
    return faceDifferencePositionF32(terms, x, y, bake.heights[2 * at + which] ?? 0);
  };
  const morphHeight = (x: number, y: number): number => bake.heights[2 * (65 * y + x) + 1] ?? 0;
  return {
    terms,
    ownOffset: (x, y) => vertex(x, y, 0),
    morphOffset: (x, y) =>
      path === "baked-offsets" ? vertex(x, y, 1) : morphOffsetFd(terms, x, y, morphHeight),
    weights: layers.weights,
    named: Array.from(
      { length: TERRAIN_CLASSES },
      (_, k) => classPhotometryOf(layers.photometry, k) !== null,
    ),
    abundances: Array.from(layers.rockAbundances ?? new Float32Array(8)),
  };
}

/** The `FaceDifferences` path's morph target (`terrainFaceDifferences.wgsl`'s `morphOffset`). */
function morphOffsetFd(
  terms: SlotTermsF32,
  x: number,
  y: number,
  height: (x: number, y: number) => number,
): Vec3 {
  const at = (px: number, py: number): Vec3 =>
    faceDifferencePositionF32(terms, px, py, height(px, py));
  const oddX = x % 2 === 1;
  const oddY = y % 2 === 1;
  if (terms.straddles || (!oddX && !oddY)) {
    return at(x, y);
  }
  const [a, b] =
    oddX && !oddY
      ? [at(x - 1, y), at(x + 1, y)]
      : !oddX
        ? [at(x, y - 1), at(x, y + 1)]
        : [at(x - 1, y - 1), at(x + 1, y + 1)];
  const f = Math.fround;
  return { x: f(0.5 * f(a.x + b.x)), y: f(0.5 * f(a.y + b.y)), z: f(0.5 * f(a.z + b.z)) };
}

/** A pebble read back from the instances: its record's values. */
function readPebbles(words: ArrayBuffer, count: number): PebbleTwinInstance[] {
  const view = new DataView(words);
  const out: PebbleTwinInstance[] = [];
  for (let n = 0; n < count; n += 1) {
    const at = n * PARTICLE_INSTANCE_BYTES;
    const radiusM = view.getFloat32(at + 12, true);
    out.push({
      centreM: {
        x: view.getFloat32(at, true),
        y: view.getFloat32(at + 4, true),
        z: view.getFloat32(at + 8, true),
      },
      radiusM,
      diameterM: 2 * radiusM,
      rotation: { x: 0, y: 0, z: 0, w: 1 },
      colour: view.getUint32(at + 24, true),
      tag: view.getUint32(at + 28, true),
    });
  }
  return out;
}

/**
 * How many of `cpu`'s pebbles a pebble of `gpu` matches: the same colour word (place and tint) and
 * patch, its centre within {@link CENTRE_ALLOWANCE_M} and its radius within
 * {@link RADIUS_ALLOWANCE}; each matched once. The radius is the fade's, so compared relatively.
 */
function matched(
  gpu: ReadonlyArray<PebbleTwinInstance>,
  cpu: ReadonlyArray<PebbleTwinInstance>,
): number {
  const byWord = new Map<string, PebbleTwinInstance[]>();
  for (const p of cpu) {
    const key = `${String(p.tag)}:${String(p.colour)}`;
    byWord.set(key, [...(byWord.get(key) ?? []), p]);
  }
  let found = 0;
  for (const p of gpu) {
    const near = byWord.get(`${String(p.tag)}:${String(p.colour)}`) ?? [];
    const k = near.findIndex(
      (q) =>
        Math.hypot(
          q.centreM.x - p.centreM.x,
          q.centreM.y - p.centreM.y,
          q.centreM.z - p.centreM.z,
        ) <= CENTRE_ALLOWANCE_M &&
        Math.abs(q.radiusM - p.radiusM) <= RADIUS_ALLOWANCE * Math.max(q.radiusM, 1e-6),
    );
    if (k >= 0) {
      found += 1;
      near.splice(k, 1);
    }
  }
  return found;
}

/** What one path's check found. */
interface PebbleFrames {
  readonly placed: boolean;
  readonly twin: boolean;
  readonly density: boolean;
  readonly frame: boolean;
  readonly detail: string;
  /** The frames without and with the pebbles, where captures were asked. */
  readonly images: ReadonlyArray<CapturedImage>;
}

/** `a` times `ka` plus `b` times `kb`. */
function along(a: Vec3, ka: number, b: Vec3, kb: number): Vec3 {
  return { x: a.x * ka + b.x * kb, y: a.y * ka + b.y * kb, z: a.z * ka + b.z * kb };
}

/** The camera, its orientation and the box hovering in view, over `centre`'s bake. */
function cameraOver(centre: BakedPatch): {
  readonly camera: Vec3;
  readonly orientation: ReturnType<typeof quaternionFromRows>;
  readonly box: ScatterBox;
  readonly forward: Vec3;
} {
  const nu = patchTerms(centre.key, MOON_LIKE, centre.originHeightM).nu0;
  const camera = along(centre.originM, 1, nu, HEIGHT_M);
  const reference = Math.abs(nu.z) > 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 0, z: 1 };
  const east = normalise(cross(reference, nu));
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
  // A 0.6 m box 1.5 m ahead along the ground, its floor 0.1 m above the datum's level there (the
  // block's ground lies within centimetres of its origin's over 1.5 m), relative to the camera.
  const ahead = along(north, 1.5, nu, 0.1 + 0.3 - HEIGHT_M);
  const box: ScatterBox = {
    centreM: ahead,
    axes: [east, north, nu],
    halfExtentsM: [0.3, 0.3, 0.3],
  };
  return { camera, orientation, box, forward };
}

/** One path's pass over the block (see the module's remarks). */
async function pebblesOn(
  engine: RenderEngine,
  path: TerrainVertexPath,
  field: WorkerFixture,
  captures: boolean,
): Promise<PebbleFrames> {
  const settings = { ...TERRAIN_SETTINGS.high, cacheBytes: CACHE_BYTES, vertexPath: path };
  const resources = new TerrainResources(engine, settings, MOON_LIKE, "lit");
  const air = createAirlessTables(engine, "scatter smoke: no air");
  const shadow = createShadowPlaceholder(engine);
  const kernels = new ScatterKernelSet(engine, path);
  const scatter = new ScatterPass(engine, `scatter smoke ${path}`, kernels, 1);
  const half = new ScatterPass(engine, `scatter smoke ${path} half`, kernels, 0.5);
  const target = engine.createRenderTarget({
    name: `scatter smoke ${path}`,
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  try {
    const keys = blockKeys();
    const bakes = await bakeOverField(
      keys,
      {
        vertexPath: path,
        normals: settings.normals,
        source: { kind: "body", body: field.body, detailSeed: field.seed, weights: "baked" },
      },
      field,
    );
    bakes.forEach((bake, slot) => {
      const result = resources.upload({ slot, ...bake });
      if (result.kind !== "uploaded") {
        throw new Error(`slot ${String(slot)} refused: ${result.reason}`);
      }
    });
    const centre = bakes[0];
    if (centre === undefined) {
      throw new Error("no centre patch was baked");
    }
    const { camera, orientation, box } = cameraOver(centre);
    const instances = new InstanceRecords(resources.layout.slots.slotCount);
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
        NO_PARENT_SLOT,
        parentQuadrant(bake.key),
      );
    });
    resources.writeFrame(instances, new ContactRecords());
    const a = MOON_LIKE.equatorialRadiusM;
    const nu = patchTerms(centre.key, MOON_LIKE, centre.originHeightM).nu0;
    const annuli = annulusEdges(0.6, 0.2, 2);
    resources.setLighting({
      lights: [
        {
          direction: normalise({ x: nu.x + 0.5, y: nu.y + 0.2, z: nu.z }),
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
    });
    const lit = resources.lit;
    const classLayers = resources.field("class-layers");
    const horizonMap = resources.field("horizon-map");
    const surveyMask = resources.field("survey-mask");
    const vertices = resources.field(path === "baked-offsets" ? "offsets" : "heights");
    const heights = resources.field("heights");
    if (
      lit === null ||
      classLayers === null ||
      horizonMap === null ||
      surveyMask === null ||
      vertices === null ||
      heights === null
    ) {
      throw new Error("the lit layout has no lit resources");
    }
    const storageBuffers: Record<string, BufferHandle> = {
      heights,
      slots: resources.slotRecords,
      instances: resources.instances,
      contacts: resources.contacts,
      classLayers,
      terrainLights: lit.lights,
      horizonMap,
      surveyMask,
    };
    if (path === "baked-offsets") {
      storageBuffers["offsets"] = vertices;
    }
    const material = await engine.createMaterialAsync(
      terrainLitMaterialSpec(path),
      ["rgba16float"],
      [resources.mesh],
    );
    const atlas = normalsAtlasOf(resources.layout);
    const litDraw: DrawItem = {
      mesh: resources.mesh,
      material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {
        bodyRotation: new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
        cameraBodyFixedM: new Float32Array([camera.x, camera.y, camera.z, 1 / a]),
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
        [DISC_NORMALS_TEXTURE]: resources.normals,
      },
      storageBuffers,
      indirect: { buffer: resources.indirect, offsetBytes: 0 },
    };
    if (!(await kernels.ready)) {
      return {
        placed: false,
        twin: false,
        density: false,
        frame: false,
        detail: `${path}: the scatter's kernels or material were refused`,
        images: [],
      };
    }
    const viewRotation = viewRotation4(orientation);
    const proj = projection(FRAME.widthPx / FRAME.heightPx);
    const pxPerRad = (FRAME.widthPx * (proj[0] ?? 1)) / 2;
    const level = finestLevel(a);
    const levels = pebbleLatticeLevels(level, PATCH_QUADS * vertexSpacing(a, level).maxM);
    const records: ScatterPatchRecord[] = bakes.map((bake, slot) => ({
      originM: {
        x: bake.originM.x - camera.x,
        y: bake.originM.y - camera.y,
        z: bake.originM.z - camera.z,
      },
      slot,
      morphStartM: 0,
      morphEndM: 0,
      parentSlot: NO_PARENT_SLOT,
      parentQuadrant: parentQuadrant(bake.key),
      key: bake.key,
      cells: [NO_PARENT_SLOT, 0, 0, 0],
    }));
    const nearestM = records.map((r, n) =>
      Math.max(
        Math.hypot(r.originM.x, r.originM.y, r.originM.z) - (bakes[n]?.boundingRadiusM ?? 0),
        0,
      ),
    );
    const order = nearestM.map((m, n) => ({ m, n })).toSorted((p, q) => p.m - q.m);
    const patches = order.flatMap(({ n }) => records[n] ?? []);
    const nearest = order.map(({ m }) => m);
    const identity: readonly [Vec3, Vec3, Vec3] = [
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 1, z: 0 },
      { x: 0, y: 0, z: 1 },
    ];
    const seed = decorationSeed(field.seed);
    const frameOf = (pass: ScatterPass): ReadonlyArray<DrawItem> =>
      pass.frame({
        patches,
        nearestM: nearest,
        levels,
        rotationRows: identity,
        pxPerRad,
        seed,
        bodies: [box],
        cull: cameraCullPassOf(viewRotation, proj, FRAME.widthPx, 1),
        viewAxis: viewAxisOf(viewRotation),
        ground: { vertices, slots: resources.slotRecords, classLayers },
        base: litDraw,
      }).draws;
    const render = async (draws: ReadonlyArray<DrawItem>): Promise<Float32Array> => {
      target.render({
        label: "scatter",
        viewRotation,
        projection: proj,
        draws,
        postProcesses: [],
        colourLoad: "clear",
      });
      return halfTexels(await engine.readTexture(target.colour));
    };
    const bare = await render([litDraw]);
    const pebbleDraws = frameOf(scatter);
    const withPebbles = await render([litDraw, ...pebbleDraws]);
    const [instanceBytes, countBytes] = await Promise.all([
      engine.readBuffer(scatter.path.instances, "tolerance"),
      engine.readBuffer(scatter.path.count, "tolerance"),
    ]);
    const count = Math.min(new Uint32Array(countBytes)[0] ?? 0, scatter.path.capacity);
    const gpu = readPebbles(instanceBytes, count);
    frameOf(half);
    const halfCount = Math.min(
      new Uint32Array(await engine.readBuffer(half.path.count, "tolerance"))[0] ?? 0,
      half.path.capacity,
    );
    const entries = scatterEntries(
      patches.map((p, n) => ({ key: p.key, nearestM: nearest[n] ?? 0 })),
      levels,
      pxPerRad,
    );
    const cpu = placePebblesCpu({
      params: {
        rotationRows: identity,
        pxPerRad,
        density: 1,
        entries: entries.length,
        capacity: scatter.path.capacity,
        seed,
        bodies: 1,
      },
      entries,
      patches,
      bodies: [box],
      slot: (slot) => {
        const bake = bakes[slot];
        if (bake === undefined) {
          throw new Error(`no bake in slot ${String(slot)}`);
        }
        return twinSlotOf(bake, path);
      },
    });
    const oversized = gpu.filter((p) => !(p.diameterM < PEBBLE_LARGEST_DIAMETER_M)).length;
    const inBox = gpu.filter((p) =>
      intersectsBody({ centreM: p.centreM, radiusM: p.radiusM }, [box]),
    ).length;
    const found = matched(gpu, cpu);
    // Half the density keeps about half as many: a binomial thinning of the full set at ½, its
    // interval 4σ, the boundary's few f32 flips aside.
    const sigma = Math.sqrt(count * 0.25);
    let changed = 0;
    let pebbleLit = 0;
    let finite = true;
    for (let p = 0; p < FRAME.widthPx * FRAME.heightPx; p += 1) {
      const moved = [0, 1, 2].some((c) => (withPebbles[4 * p + c] ?? 0) !== (bare[4 * p + c] ?? 0));
      if (moved) {
        changed += 1;
        if (withPebbles[4 * p + 3] === METER_CLASS.litBody && (withPebbles[4 * p + 1] ?? 0) > 0) {
          pebbleLit += 1;
        }
      }
    }
    finite = withPebbles.every(Number.isFinite);
    return {
      placed: count > 0 && oversized === 0 && inBox === 0 && pebbleDraws.length > 0,
      twin:
        Math.abs(count - cpu.length) <= Math.max(2, COUNT_ALLOWANCE * cpu.length) &&
        found >= MATCHED_SHARE * Math.min(count, cpu.length),
      density: Math.abs(halfCount - count / 2) <= 4 * sigma + 2,
      frame: finite && changed > 0 && pebbleLit > 0.5 * changed,
      images: captures
        ? [
            hdrCapture(`r11-t6-ground-${path}`, bare, FRAME, 1, 2),
            hdrCapture(`r11-t6-pebbles-${path}`, withPebbles, FRAME, 1, 2),
          ]
        : [],
      detail: `${path}: ${String(entries.length)} entries over ${String(patches.length)} patches, k ${Array.from(
        centre.layers?.rockAbundances ?? [],
      )
        .map((k) => k.toFixed(3))
        .join(
          "/",
        )}; ${String(count)} pebbles on the GPU against the twin's ${String(cpu.length)}, ${String(found)} matched; ${String(oversized)} of 0.4 m or more, ${String(inBox)} in the box; at half the density ${String(halfCount)}; ${String(changed)} pixels moved, ${String(pebbleLit)} of them lit, finite ${String(finite)}`,
    };
  } finally {
    target.dispose();
    scatter.dispose();
    half.dispose();
    kernels.dispose();
    resources.dispose();
    engine.releaseTexture(air.body);
    engine.releaseTexture(shadow);
  }
}

/**
 * The pass over a generated world on each vertex path (see the module's remarks), with its frames
 * as captures where `captures` asks them.
 */
export async function checkPebbles(
  engine: RenderEngine,
  checks: Checks,
  captures = false,
): Promise<CapturedImage[]> {
  const field = moonLikeFixture("first");
  const images: CapturedImage[] = [];
  for (const path of ["baked-offsets", "face-differences"] as const) {
    // Each path's frames run in order: each bakes, draws and reads back before the next allocates.
    // oxlint-disable-next-line no-await-in-loop
    const found = await pebblesOn(engine, path, field, captures);
    images.push(...found.images);
    checks.check(
      `R11.T6.a the scatter places pebbles below 0.4 m, none in a body's box (${path})`,
      found.placed,
      found.detail,
    );
    checks.check(
      `R11.T6.b the pebbles' placement against its twin (${path})`,
      found.twin,
      found.detail,
    );
    checks.check(
      `R11.T6.b half the scatter density keeps half the pebbles (${path})`,
      found.density,
      found.detail,
    );
    checks.check(
      `R11.T6.a the pebbles drawn over the lit terrain, finite and lit (${path})`,
      found.frame,
      found.detail,
    );
  }
  return images;
}

/** The cull's twin's points: spheres about two boxes, inside, straddling a face and beside it. */
function cullPoints(
  boxes: ReadonlyArray<ScatterBox>,
): Array<{ readonly centreM: Vec3; readonly radiusM: number }> {
  const out: Array<{ readonly centreM: Vec3; readonly radiusM: number }> = [];
  for (const box of boxes) {
    for (const [k, axis] of box.axes.entries()) {
      const half = box.halfExtentsM[k] ?? 0;
      for (const sign of [-1, 1]) {
        // Centres beyond the face by a quarter, a half and twice the radius, and inside.
        for (const [beyond, radius] of [
          [0.025, 0.1],
          [0.05, 0.1],
          [0.2, 0.1],
          [-0.1, 0.05],
          [0.0049, 0.005],
          [0.0101, 0.01],
        ] as const) {
          const d = sign * (half + beyond);
          out.push({
            centreM: {
              x: box.centreM.x + d * axis.x,
              y: box.centreM.y + d * axis.y,
              z: box.centreM.z + d * axis.z,
            },
            radiusM: radius,
          });
        }
      }
    }
  }
  return out;
}

/** The cull against its twin (see the module's remarks). */
export async function checkScatterCull(engine: RenderEngine, checks: Checks): Promise<void> {
  const tilt = normalise({ x: 0.2, y: 0.1, z: 1 });
  const east = normalise(cross({ x: 0, y: 1, z: 0 }, tilt));
  const north = cross(tilt, east);
  const boxes: ScatterBox[] = [
    // A tilted craft's hull on the ground.
    { centreM: { x: 3, y: -2, z: 1.2 }, axes: [east, north, tilt], halfExtentsM: [2.5, 1.5, 1.2] },
    // A body hovering 0.1 m up: its floor at z = 0.1.
    {
      centreM: { x: -4, y: 1, z: 0.6 },
      axes: [
        { x: 1, y: 0, z: 0 },
        { x: 0, y: 1, z: 0 },
        { x: 0, y: 0, z: 1 },
      ],
      halfExtentsM: [1, 1, 0.5],
    },
  ];
  const points = cullPoints(boxes);
  // Pebbles on the ground under the hovering body: one that reaches its floor and one that does not.
  points.push(
    { centreM: { x: -4, y: 1, z: 0 }, radiusM: 0.12 },
    { centreM: { x: -4, y: 1, z: 0 }, radiusM: 0.05 },
  );
  const kernel = await engine.createComputeAsync(SCATTER_CULL_TWIN_KERNEL);
  const pointData = new Float32Array(4 * points.length);
  points.forEach((p, n) => {
    pointData.set([p.centreM.x, p.centreM.y, p.centreM.z, p.radiusM], 4 * n);
  });
  const pointBuffer = engine.createBuffer({
    name: "scatter cull twin points",
    bytes: pointData.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "scatter",
  });
  const met = engine.createBuffer({
    name: "scatter cull twin met",
    bytes: 4 * points.length,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "scatter",
  });
  try {
    engine.writeBuffer(pointBuffer, 0, pointData);
    const packed = new Float32Array((MAX_SCATTER_BODIES * SCATTER_BOX_BYTES) / 4);
    const count = packScatterBoxes(boxes, packed);
    engine.dispatch(
      kernel,
      {
        uniforms: { params: Uint32Array.of(count, points.length, 0, 0), bodies: packed },
        buffers: { points: pointBuffer, met },
        sampled: {},
        storage: {},
      },
      scatterCullTwinWorkgroups(points.length),
      "scatter cull twin",
    );
    const read = new Uint32Array(await engine.readBuffer(met));
    const expected = points.map((p) => (intersectsBody(p, boxes) ? 1 : 0));
    const differ = expected.filter((e, n) => e !== read[n]).length;
    const culled = expected.filter((e) => e === 1).length;
    const under = [read[points.length - 2], read[points.length - 1]];
    checks.check(
      "R11.T6.b the scatter's cull against its twin, a sphere straddling a face and a body hovering 0.1 m up",
      differ === 0 && under[0] === 1 && under[1] === 0 && culled > 0 && culled < points.length,
      `${String(points.length)} spheres, ${String(culled)} culled by the twin, ${String(differ)} differ; under the hovering body ${String(under[0])} (reaches its floor) and ${String(under[1])} (does not)`,
    );
  } finally {
    engine.releaseBuffer(pointBuffer);
    engine.releaseBuffer(met);
    engine.releaseCompute(kernel);
  }
}
