/**
 * A lit pass's shadow cascades on the high setting (plan R10, R10.T9.b; Design note 11;
 * decision-r10-t9b-design D2 to D6 and D9): the set made at the first frame that draws them, the
 * placeholder before, the lit draw made again as the bound texture changes, each view's four caster
 * passes of its own records, culled by the cascade's light-space box and the caster reach, the
 * view's snapped centres carried and dropped, the horizon texel from the view's morph bands, the
 * other casters' draws, and no pass at w = 0, over a scripted pool and R01's counting engine; and
 * the far cascades at a lower rate (plan R14, R14.T6.b): held between their frames, sampled as their
 * own frames drew them, and rendered again at every cut.
 */

import { describe, expect, it, vi } from "vitest";

import { instrumentViewId, PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../geometry/vec3";
import { CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { FakeAdapter, SWIFTSHADER_INFO } from "../../test/fakeGpu";
import { lavaWorldSurface } from "../../test/thermalSurfaces";
import { surfaceTemperatureFieldOf } from "../atmosphere/thermal";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lawFor } from "../appearance/phase";
import {
  lookAlong,
  multiply,
  quaternionFromAxisAngle,
  quaternionFromRows,
  rotationRows,
} from "../camera/quaternion";
import type { ViewId } from "../camera/state";
import { IDENTITY_ROTATION, rotateToBody, rotation3FromRows } from "../coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { DepthLayerPass, DrawItem, MaterialHandle, MeshHandle } from "../engine/types";
import { annulusEdges } from "../lighting/annuli";
import { BUCKET_ARGS_BYTES } from "../clusters/buckets";
import { compactTwin } from "../clusters/compact";
import {
  type PyramidFrame,
  pyramidLevels,
  pyramidProjectionOf,
  pyramidTexels,
} from "../clusters/hzb";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import {
  cascadeKeeps,
  CLUSTER_CULL_KERNEL,
  CLUSTER_CULL_OCCLUSION_KERNEL,
  CULL_INPUT_WHOLE,
  cullMarksTwin,
  cullOverTwin,
  cullTwin,
  NO_MARK,
} from "../clusters/cull";
import { type CameraCullPass, type CascadeCullPass, CULL_PASS_BYTES } from "../clusters/cullPass";
import {
  CLUSTER_FLAG_MAY_DISCARD,
  CLUSTER_FLAG_MEASURED,
  type ClusterRecord,
} from "../clusters/record";
import {
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_INPUT_COLUMN_RUN_SHARED,
  TERRAIN_INPUT_EDGE_MASK,
  TERRAIN_INPUT_ROW_RUN_SHARED,
  TERRAIN_INPUT_STAND_IN_BITS,
  TERRAIN_INPUT_SURVEYED,
  TERRAIN_MASK_LEVELS,
  TERRAIN_PAGE_CLUSTERS,
  TERRAIN_SUB_PATCH_MASK,
} from "../clusters/terrain";
import {
  EDGE_BANDS,
  edgeBandIndex,
  edgeBandMOfHalf,
  INSTANCE_FINEST,
  INSTANCE_SUB_GRID_STRIDES,
  NO_STRIDE_BAND,
  NO_STRIDE_SCALE,
  type StrideScale,
  strideNeedM,
} from "../clusters/strides";
import { STRIDES } from "../clusters/templates";
import {
  GEOMETRY_BEFORE_R14,
  type GeometrySettings,
  SETTINGS,
  TERRAIN_SETTINGS,
} from "../quality/qualitySetting";
import { vertexDir, vertexSpacing } from "./cube";
import { horizonCone } from "./cull";
import { allocationTally } from "./gpu/allocationTally";
import { TERRAIN_SHADOW_PASS_LABEL } from "./gpu/material";
import {
  CLUSTER_HISTORY_SLOT_BYTES,
  CLUSTER_LIT_LATE_PASS,
  CLUSTER_LIT_PASS,
  LIT_PATCH_INDICES,
  normalsAtlasOf,
  TERRAIN_CLUSTER_MESH,
} from "./gpu/resources";
import {
  DATUM_SLOT,
  fadeWord,
  INSTANCE_FADES_WORD,
  INSTANCE_PARENT_SLOT_WORD,
  INSTANCE_EDGE_BANDS_WORD,
  INSTANCE_RECORD_BYTES,
  INSTANCE_SLOT_WORD,
  INSTANCE_SUB_PATCH_WORD,
  mixStep,
  NO_PARENT_SLOT,
  PARENT_MIX_SLOT,
} from "./gpu/uniforms";
import { MAX_LEVEL, type PatchKey } from "./patchKey";
import { type PlanetGeometry, planetGeometry, spheroidNormal, surfacePoint } from "./planet";
import type { GroundContact } from "./grounded";
import type { PatchRequest } from "./select";
import type { CascadeIndex } from "./shadows/cascadeShadow";
import {
  CASCADE_BYTES,
  type CascadeSet,
  discTopDirection,
  fitCascades,
  gridLatticeSpacingM,
} from "./shadows/cascades";
import { CascadeReuse, SHADOW_PLACEHOLDER_NAME, TerrainShadows } from "./shadows/terrainShadows";
import type { TerrainLighting } from "./terrainMaterial";
import {
  appendCasterRecord,
  cascadeHorizonTexelM,
  BAND_SLEW_PER_STEP,
  castsInto,
  effectiveTauPx,
  meanRadiusM,
  morphRangeM,
  TerrainPass,
  type TerrainPassOptions,
  type TerrainPassView,
  type TerrainPool,
  type TerrainView,
  type TerrainViewFrame,
  type TerrainViewFrameInput,
  TERRAIN_CLUSTER_COMPACT_LABEL,
  TERRAIN_CLUSTER_CULL_LABEL,
} from "./terrainPass";
import {
  type BakedPatch,
  type BakeSettings,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

/** The lit pass's cull, the occlusion's variant (plan R14, R14.T3.d), by its name. */
const LIT_CULL = CLUSTER_CULL_OCCLUSION_KERNEL.name;

const PLANET: PlanetGeometry = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

const PRIMARY = PRIMARY_VIEW_ID;
const INSTRUMENT = instrumentViewId(1);

const RAD = Math.PI / 180;

/**
 * A body's layers, complete and surveyed but at one corner vertex, so that its patches are drawn
 * by the lit mesh, as edge patches (plan R14, R14.T1.a).
 */
function layers(): TerrainLayers {
  const palette = new Uint8Array(16).fill(PALETTE_NONE);
  palette.set([1, 0, 2, PALETTE_NONE]);
  const photometry = new Float32Array(40);
  photometry.set([0.1, 0.12, 0.14, 1, 1]);
  const surveyMask = new Uint8Array(65 * 65).fill(255);
  surveyMask[0] = 0;
  return {
    palette,
    weights: new Uint8Array(65 * 65 * 8),
    surveyMask,
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    photometry,
    horizonNear: new Uint32Array(65 * 65 * 4),
    horizonFar: new Uint32Array(9 * 9 * 4),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/** A pool that bakes each demanded patch flat on the datum, at its own place, on the test's call. */
class PlacedLane implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  readonly #baked = new Set<(bake: BakedPatch) => void>();
  /** The layers each bake carries: {@link layers}' unless a test gives others. */
  layers: TerrainLayers = layers();
  /** The measured errors each bake carries (plan R14, R14.T3.b), none unless a test gives them. */
  clusterErrors: Float16Array | null = null;

  constructor(readonly settings: BakeSettings) {}

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#baked.add(cb);
    return () => {
      this.#baked.delete(cb);
    };
  }

  terminate(): void {
    this.#baked.clear();
  }

  /** Answers every demanded patch. */
  bakeDemand(pass: TerrainPass): void {
    for (const request of this.demand) {
      this.deliver(pass, request.key);
    }
  }

  /** Bakes `key` and hands it to `pass`. */
  deliver(pass: TerrainPass, key: PatchKey): void {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    const [x, y, z] = surfacePoint(PLANET.figure, vertexDir(key, 32, 32), 0);
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: { x, y, z },
      heights: new Float32Array(65 * 65 * 2),
      offsets: this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
      normals: new Float16Array(n * n * 2),
      heightRangeM: [0, 0],
      // Half the patch's diagonal at its largest spacing, and more.
      boundingRadiusM: 64 * vertexSpacing(PLANET.figure.equatorialRadiusM, key.level).maxM,
      originHeightM: 0,
      skirtDepthM: 1,
      layers: this.layers,
      ...(this.clusterErrors === null ? {} : { clusterErrors: this.clusterErrors }),
    };
    for (const listener of this.#baked) {
      listener(bake);
    }
  }
}

/** A body's lit source, its bakes with their weights. */
const BODY = {
  kind: "body",
  body: "0200080020000000.0301",
  detailSeed: "617373656d626c79",
  weights: "baked",
} as const;

/** The place the views stand over: 30° N, 20° E, body-fixed, as a unit direction. */
const OVER: readonly [number, number, number] = [
  Math.cos(30 * RAD) * Math.cos(20 * RAD),
  Math.cos(30 * RAD) * Math.sin(20 * RAD),
  Math.sin(30 * RAD),
];

/** The datum's normal and its local east and north there. */
const FRAME_THERE = (() => {
  const [x, y, z] = spheroidNormal(PLANET.figure, OVER);
  const up: Vec3 = { x, y, z };
  const east = normalise(cross({ x: 0, y: 0, z: 1 }, up));
  return { up, east, north: cross(up, east) };
})();

/** The sun 15° up towards the east, body-fixed. */
const SUN: Vec3 = add(
  scale(FRAME_THERE.east, Math.cos(15 * RAD)),
  scale(FRAME_THERE.up, Math.sin(15 * RAD)),
);

/**
 * A view `heightM` over the place, looking `depressionDeg` below the horizon `headingDeg` east of
 * north; the body does not turn.
 */
function viewOver(heightM: number, headingDeg = 0, depressionDeg = 10): TerrainView {
  const [x, y, z] = surfacePoint(PLANET.figure, OVER, heightM);
  const { up, east, north } = FRAME_THERE;
  const ahead = add(
    scale(north, Math.cos(headingDeg * RAD)),
    scale(east, Math.sin(headingDeg * RAD)),
  );
  const forward = add(
    scale(ahead, Math.cos(depressionDeg * RAD)),
    scale(up, -Math.sin(depressionDeg * RAD)),
  );
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: { x, y, z },
    orientation: lookAlong(forward, up),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 640, heightPx: 360 },
  };
}

/** One Sun-like star, `SUN`, at 1 au (IAU 2012 B2) with the Sun's radius (IAU 2015 B3). */
const STARLIT: TerrainLighting = (() => {
  const a = PLANET.figure.equatorialRadiusM;
  const annuli = annulusEdges(0.6, 0.2, 2);
  return {
    lights: [
      {
        direction: SUN,
        distance: 1.495978707e11 / a,
        radius: 6.957e8 / a,
        illuminance: [1.3e5, 1.25e5, 1.2e5],
        annuli: [annuli, annuli, annuli],
      },
    ],
    occluders: [],
    secondaries: [],
    elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
    exposureScale: 1e-4,
  };
})();

/** The star's angular radius from the body's centre, rad: asin(min(1, radius ÷ distance)), R07's. */
const STAR_RHO_RAD = (() => {
  const light = STARLIT.lights[0];
  if (light === undefined) {
    throw new Error("the cases' lighting has a star");
  }
  return Math.asin(Math.min(1, light.radius / light.distance));
})();

/** A view's frame inputs under the star, its body taking the cascades or not. */
function frameOf(view: TerrainView, takesCascades: boolean, nowMs = 0): TerrainViewFrameInput {
  return {
    view,
    sunDirectionBodyFixed: SUN,
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-4,
    lighting: STARLIT,
    takesCascades,
    nowMs,
  };
}

/** A view of the pass at the high setting's τ. */
function passView(id: ViewId, view: TerrainView): TerrainPassView {
  return { id, view, tauPx: TERRAIN_SETTINGS.high.tauPx, weight: 1 };
}

interface Made {
  readonly pass: TerrainPass;
  readonly engine: CountingRenderEngine;
  readonly shadows: TerrainShadows;
  readonly lane: () => PlacedLane;
}

/** A lit pass on `setting` with a session's cascades of `engine` and the body's `reliefM`, ready. */
async function litPass(
  setting: "high" | "low" = "high",
  given?: CountingRenderEngine,
  reliefM: () => number | null = () => 8_000,
  extra: Pick<TerrainPassOptions, "clusterCullKernel"> = {},
): Promise<Made> {
  const engine =
    given ??
    (await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    }));
  const lanes: PlacedLane[] = [];
  const shadows = new TerrainShadows(engine, { cascades: setting === "high" });
  const pass = new TerrainPass({
    engine,
    setting,
    planet: PLANET,
    source: BODY,
    createPool: (bake) => {
      const lane = new PlacedLane(bake);
      lanes.push(lane);
      return lane;
    },
    shadows,
    reliefM,
    ...extra,
  });
  await pass.ready();
  return {
    pass,
    engine,
    shadows,
    lane: () => {
      const lane = lanes.at(-1);
      if (lane === undefined) {
        throw new Error("no lane was taken");
      }
      return lane;
    },
  };
}

/** Steps `made` over `views` and bakes its demand, `rounds` times, then steps once more. */
function stream(made: Made, views: ReadonlyArray<TerrainPassView>, rounds = 4): void {
  for (let n = 0; n < rounds; n += 1) {
    made.pass.step({ views, grounded: [] });
    made.lane().bakeDemand(made.pass);
  }
  made.pass.step({ views, grounded: [] });
}

/** A streamed pass with the primary at `view`. */
async function streamedAt(view: TerrainView, setting: "high" | "low" = "high"): Promise<Made> {
  const made = await litPass(setting);
  stream(made, [passView(PRIMARY, view)]);
  return made;
}

/** The primary 50 m over the place, looking north 10° below the horizon. */
const NEAR = viewOver(50);

/**
 * The primary 10⁶ km over the place, looking all but straight down: far enough that cascade 1's
 * texel (some 880 km) is coarser than the coarsest level's spacing (145 km), so w = 0. Nearer, the
 * horizon texel the view's bands give is coarse enough that w > 0 even from 2,000 km at τ = 1 px.
 */
const ORBIT = viewOver(1e9, 0, 89.5);

/** A frame's draw, which it must have. */
function drawOf(frame: TerrainViewFrame): DrawItem {
  const draw = frame.draws.at(-1);
  if (draw === undefined) {
    throw new Error("the pass drew nothing");
  }
  return draw;
}

/** A frame's cascades, which it must have. */
function cascadesOf(frame: TerrainViewFrame): NonNullable<TerrainViewFrame["cascades"]> {
  if (frame.cascades === null) {
    throw new Error("the frame has no cascades");
  }
  return frame.cascades;
}

/** The records last written to `buffer`, one hex string a record. */
function recordsIn(engine: CountingRenderEngine, buffer: string): string[] {
  const data = engine.writes.findLast((w) => w.buffer === buffer)?.data ?? new Uint8Array();
  const out: string[] = [];
  for (let at = 0; at + INSTANCE_RECORD_BYTES <= data.byteLength; at += INSTANCE_RECORD_BYTES) {
    out.push(
      Array.from(data.slice(at, at + INSTANCE_RECORD_BYTES), (byte) =>
        byte.toString(16).padStart(2, "0"),
      ).join(""),
    );
  }
  return out;
}

/** Whether `part` is `whole` with some records left out, in order. */
function subsequence(part: ReadonlyArray<string>, whole: ReadonlyArray<string>): boolean {
  let at = 0;
  for (const record of part) {
    at = whole.indexOf(record, at);
    if (at < 0) {
      return false;
    }
    at += 1;
  }
  return true;
}

/** The terrain's caster draw of `pass`, the first of its draws where it has casters. */
function terrainCaster(pass: DepthLayerPass): DrawItem {
  const draw = pass.draws[0];
  if (draw === undefined || !draw.material.name.startsWith("terrain-shadow-")) {
    throw new Error(`pass ${pass.layer} draws no terrain`);
  }
  return draw;
}

/** The `u32` words last written to `buffer`. */
function wordsIn(engine: CountingRenderEngine, buffer: string): number[] {
  const data = engine.writes.findLast((w) => w.buffer === buffer)?.data ?? new Uint8Array();
  return [...new Uint32Array(data.slice().buffer)];
}

/** A `u32`'s bits as an `f32`. */
function f32Of(bits: number): number {
  return new Float32Array(Uint32Array.of(bits).buffer)[0] ?? Number.NaN;
}

/** A clustered pass's list entries a pass, from its slots. */
function PASS_ENTRIES(made: Made): number {
  // Its blocks and strips at four strides, then each shape's may-discard template (R14.T1.g).
  return made.pass.layout.slots.slotCount * (4 * TERRAIN_CLUSTERS_PER_PAGE + TERRAIN_PAGE_CLUSTERS);
}

/** A streamed pass at {@link NEAR} whose every bake is wholly surveyed: its patches interior. */
async function clusteredPass(setting: "high" | "low" = "high"): Promise<Made> {
  const made = await litPass(setting);
  // At stride 1, as the cluster path's own checks draw.
  made.pass.setGeometry({ ...SETTINGS[setting].geometry, strides: "off" });
  // Its ground's changes cut at their first draw, so that no record's heights fade, which the cull
  // keeps whole (R10.T16.i; R14.T1.g).
  made.pass.setReducedMotion(true);
  made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
  // Until nothing more is asked for, so that no patch is drawn by a stand-in.
  stream(made, [passView(PRIMARY, NEAR)], 24);
  return made;
}

/**
 * The high setting's geometry with every cluster at stride 1 (R14.T3.c's `strides` off): the
 * cluster path's draws and lists as T1 and T2 check them.
 */
const HIGH_AT_STRIDE_1: GeometrySettings = { ...SETTINGS.high.geometry, strides: "off" };

/** The high setting's geometry with the lit pass by the interior mesh: T1.e's overlay alone. */
const OVERLAY_ALONE: GeometrySettings = { ...SETTINGS.high.geometry, litClusters: false };

/**
 * The high setting's geometry without T1.c's receivers' cull, at stride 1, the overlay redrawn and
 * the lit pass by the interior mesh: T1.b's path alone.
 */
const CLUSTERS_ALONE: GeometrySettings = {
  ...SETTINGS.high.geometry,
  casterDepthCull: false,
  casterStrideCap: 1,
  overlayDepth: "redraw",
  litClusters: false,
  farCascadeRate: 1,
};

/** A draw as the names of what it draws with: the item a fresh pass's would be. */
function drawShape(draw: DrawItem): unknown {
  return {
    mesh: draw.mesh.name,
    material: draw.material.name,
    storage: Object.fromEntries(
      Object.entries(draw.storageBuffers ?? {}).map(([name, buffer]) => [name, buffer.name]),
    ),
    indirect:
      draw.indirect === undefined ? null : [draw.indirect.buffer.name, draw.indirect.offsetBytes],
    base: [...(draw.uniforms["clusterBase"] ?? [])],
  };
}

/** The cull's ordered camera pass (kind 3, plan R14, R14.T2.a) at `index`, back as the twin takes it. */
function parseLitPass(words: Uint32Array | Float32Array, index: number): CameraCullPass {
  const view = new DataView(words.buffer, words.byteOffset, words.byteLength);
  const at = index * CULL_PASS_BYTES;
  if (view.getUint32(at, true) !== 3) {
    throw new Error(`the cull's pass ${String(index)} is not an ordered camera pass`);
  }
  const f = (o: number): number => view.getFloat32(at + o, true);
  const vec = (o: number): Vec3 => ({ x: f(o), y: f(o + 4), z: f(o + 8) });
  return {
    kind: "camera",
    planes: Array.from({ length: f(20) }, (_, k) => ({
      normal: vec(32 + 16 * k),
      offsetM: f(44 + 16 * k),
    })),
    toleranceRad: f(16),
    strideCap: STRIDES[view.getUint32(at + 4, true)] ?? 1,
    horizon: f(28) > 0 ? { axis: vec(112), planeM: f(24), sin: f(28), cos: f(124) } : null,
    order: "ordered",
  };
}

/** The copies of the cull's counts of strides past τ′ `made` has dispatched (plan R14, R14.T3.e). */
function overCopies(made: Made): number {
  return made.engine.dispatched.filter((d) => d.kernel === "cluster over counts").length;
}

/** The high setting's geometry with every cascade rendered every frame (plan R14, R14.T6.b). */
const EVERY_FRAME: GeometrySettings = { ...SETTINGS.high.geometry, farCascadeRate: 1 };

/** The cull's passes, as `packCullPasses` laid them out, back as the twin takes them. */
function parsePasses(words: Uint32Array | Float32Array): CascadeCullPass[] {
  const view = new DataView(words.buffer, words.byteOffset, words.byteLength);
  const out: CascadeCullPass[] = [];
  for (let at = 0; at + CULL_PASS_BYTES <= view.byteLength; at += CULL_PASS_BYTES) {
    if (view.getUint32(at, true) !== 1) {
      continue;
    }
    const f = (o: number): number => view.getFloat32(at + o, true);
    const axis = (o: number): Vec3 => ({ x: f(o), y: f(o + 4), z: f(o + 8) });
    out.push({
      kind: "cascade",
      axes: [axis(32), axis(48), axis(64)],
      centreFromCameraLightM: axis(80),
      radiusM: f(20),
      depthFarM: f(24),
      toleranceM: f(16),
      strideCap: STRIDES[view.getUint32(at + 4, true)] ?? 1,
      receivers:
        f(28) === 0
          ? null
          : {
              depthM: [f(96), f(100)],
              reachM: [f(104), f(108)],
              coveredBelow:
                f(120) < 0
                  ? null
                  : { centreM: [f(112), f(116)], halfSideM: f(120), reachM: f(124) },
            },
    });
  }
  return out;
}

/** The cull's view axis, as `packCullParams` laid it out. */
function parsedViewAxis(words: Uint32Array | Float32Array): Vec3 {
  const view = new DataView(words.buffer, words.byteOffset, words.byteLength);
  return {
    x: view.getFloat32(64, true),
    y: view.getFloat32(68, true),
    z: view.getFloat32(72, true),
  };
}

/** The instances' origins and pages as last written to `buffer`. */
function instanceOrigins(
  engine: CountingRenderEngine,
  buffer: string,
): Array<{ readonly originM: Vec3; readonly page: number }> {
  const data = engine.writes.findLast((w) => w.buffer === buffer)?.data ?? new Uint8Array();
  const view = new DataView(data.slice().buffer);
  const out: Array<{ readonly originM: Vec3; readonly page: number }> = [];
  for (let at = 0; at + INSTANCE_RECORD_BYTES <= view.byteLength; at += INSTANCE_RECORD_BYTES) {
    out.push({
      originM: {
        x: view.getFloat32(at, true),
        y: view.getFloat32(at + 4, true),
        z: view.getFloat32(at + 8, true),
      },
      page: view.getUint32(at + 12, true),
    });
  }
  return out;
}

/** A pass's templates, its buckets a pass: the strided eight and the may-discard four (R14.T1.g). */
const TEMPLATES = TERRAIN_CLUSTER_MESH.templates.length;

/** A record's shape by its flags' code (`record.ts`' `CLUSTER_SHAPE_CODE`). */
const SHAPES: ReadonlyArray<ClusterRecord["shape"]> = [
  "terrain-block",
  "terrain-skirt",
  "mesh",
  "terrain-fin",
  "terrain-head",
];

/** Each slot's cluster records as last written, by slot and cluster. */
function clusterRecordsOf(
  engine: CountingRenderEngine,
): (page: number, cluster: number) => ClusterRecord {
  const bySlot = new Map<number, DataView>();
  for (const write of engine.writes) {
    if (write.buffer === "terrain cluster records") {
      bySlot.set(
        write.offsetBytes / (TERRAIN_PAGE_CLUSTERS * 32),
        new DataView(write.data.slice().buffer),
      );
    }
  }
  return (page, cluster) => {
    const view = bySlot.get(page);
    if (view === undefined) {
      throw new Error(`slot ${String(page)} has no cluster records`);
    }
    const at = cluster * 32;
    return {
      centreM: {
        x: view.getFloat32(at, true),
        y: view.getFloat32(at + 4, true),
        z: view.getFloat32(at + 8, true),
      },
      radiusM: view.getFloat32(at + 12, true),
      mode: "strided",
      strideErrorBits: null,
      templateRef: view.getUint32(at + 24, true),
      shape: SHAPES[view.getUint32(at + 28, true) & 0xf] ?? "terrain-block",
      mayDiscard: (view.getUint32(at + 28, true) & CLUSTER_FLAG_MAY_DISCARD) !== 0,
      material: 0,
    };
  };
}

/** Whether `texels` is a whole number of texels, to 10⁻⁶ texel. */
function wholeTexels(texels: number): boolean {
  return Math.abs(texels - Math.round(texels)) < 1e-6;
}

/**
 * Whether each of `set`'s cascades is whole texels in x_L and y_L from the body-fixed lattice point
 * nearest the camera: a first frame's snapping, with no centres carried (ruling 7 amended).
 */
function onLattice(set: CascadeSet, cameraM: Vec3): boolean {
  const spacingM = gridLatticeSpacingM(set.cascades[0].radiusM);
  const round = (c: number): number => spacingM * Math.floor(c / spacingM + 0.5);
  const origin = { x: round(cameraM.x), y: round(cameraM.y), z: round(cameraM.z) };
  return set.cascades.every((cascade) => {
    const fromOrigin = sub(cascade.centreBodyFixedM, origin);
    const along = (axis: Vec3): number =>
      (fromOrigin.x * axis.x + fromOrigin.y * axis.y + fromOrigin.z * axis.z) / cascade.texelM;
    return wholeTexels(along(set.basis.x)) && wholeTexels(along(set.basis.y));
  });
}

describe("the set of cascades and its placeholder in a lit pass (R10.T9.b; D2)", () => {
  it("makes the set once, at the first frame its body takes the cascades at w > 0, its 67,108,864 B under shadows", async () => {
    const made = await streamedAt(NEAR);
    const tally = allocationTally(made.engine);
    const view = viewOver(50);
    made.pass.drawFor(PRIMARY, frameOf(view, false));
    const before = [tally.liveBytes("shadows"), made.engine.counts.depthLayerTargets];
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, true));
    made.pass.drawFor(PRIMARY, frameOf(view, true, 16));
    expect([
      before,
      cascadesOf(frame).set.weight > 0,
      tally.liveBytes("shadows"),
      made.engine.counts.depthLayerTargets,
    ]).toEqual([[0, 0], true, CASCADE_BYTES, 1]);
  });

  it("binds the placeholder before the set and the set after, making its lit draw again only as that changes", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const first = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, false)));
    const second = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, false)));
    const withSet = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    const again = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    const without = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, false)));
    expect([
      first.textures["shadowCascades"]?.name,
      second === first,
      withSet === first,
      withSet.textures["shadowCascades"] === made.shadows.target?.depth,
      again === withSet,
      without === withSet,
      // A body that takes no cascades draws at w = 0, whatever its draw binds.
      without.uniforms["cascadeViewAxis"]?.[3],
    ]).toEqual([SHADOW_PLACEHOLDER_NAME, true, false, true, true, true, 0]);
  });

  it("makes its lit draw again with the set remade after a restore, its centres and its records new", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const before = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    const target = made.shadows.target;
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    stream(made, [passView(PRIMARY, view)]);
    const after = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    expect([
      after === before,
      made.shadows.target === target,
      after.textures["shadowCascades"] === made.shadows.target?.depth,
      made.engine.counts.depthLayerTargets,
    ]).toEqual([false, false, true, 2]);
  });

  it("makes no set on the low setting and binds the placeholder, whatever its body takes", async () => {
    const made = await streamedAt(NEAR, "low");
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    expect([
      frame.cascades,
      drawOf(frame).textures["shadowCascades"]?.name,
      made.engine.counts.depthLayerTargets,
      made.engine.depthLayerRenders.length,
    ]).toEqual([null, SHADOW_PLACEHOLDER_NAME, 0, 0]);
  });

  it("makes its own cascades where it is given none, and lets them go with itself", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const tally = allocationTally(engine);
    const before = engine.live;
    const lanes: PlacedLane[] = [];
    const pass = new TerrainPass({
      engine,
      setting: "high",
      planet: PLANET,
      source: BODY,
      createPool: (bake) => {
        const lane = new PlacedLane(bake);
        lanes.push(lane);
        return lane;
      },
    });
    await pass.ready();
    const views = [passView(PRIMARY, NEAR)];
    for (let n = 0; n < 4; n += 1) {
      pass.step({ views, grounded: [] });
      lanes.at(-1)?.bakeDemand(pass);
    }
    pass.step({ views, grounded: [] });
    pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const living = tally.liveBytes("shadows");
    pass.dispose();
    expect([living, tally.liveBytes("shadows"), engine.live]).toEqual([CASCADE_BYTES, 0, before]);
  });

  it("lets its own cascades and their restore listener go where its device cannot be made, as during a loss", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const listening = engine.restoredListenerCount;
    engine.loseDevice();
    expect(
      () =>
        new TerrainPass({
          engine,
          setting: "high",
          planet: PLANET,
          source: BODY,
          createPool: (bake) => new PlacedLane(bake),
        }),
    ).toThrow(EngineUnavailable);
    expect(engine.restoredListenerCount).toBe(listening);
  });
});

describe("each view's caster passes (R10.T9.b; D1 and D4)", () => {
  it("draws four passes a view-frame, each into its own layer with its cascade's matrices, under the view's name", async () => {
    const made = await streamedAt(NEAR);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    const { set, passes } = cascadesOf(frame);
    const render = made.engine.depthLayerRenders.at(-1);
    expect([
      made.engine.depthLayerRenders.length,
      render?.owner,
      render?.target,
      render?.passes === passes,
      passes.map((each) => [each.label, each.layer]),
      ([0, 1, 2, 3] as const).every(
        (n) =>
          passes[n]?.viewRotation === set.cascades[n].viewRotation &&
          passes[n].projection === set.cascades[n].projection,
      ),
    ]).toEqual([
      1,
      PRIMARY,
      "terrain shadow cascades",
      true,
      [0, 1, 2, 3].map((layer) => [TERRAIN_SHADOW_PASS_LABEL, layer]),
      true,
    ]);
  });

  it("draws the terrain's casters with the view's own contacts, storage, Draw values and lit mesh, through the caster, a direct count", async () => {
    const made = await streamedAt(NEAR);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    const lit = drawOf(frame);
    const caster = terrainCaster(cascadesOf(frame).passes[0] ?? fail());
    const storage = caster.storageBuffers ?? {};
    const litStorage = lit.storageBuffers ?? {};
    expect({
      material: caster.material.name,
      mesh: caster.mesh === lit.mesh,
      shared: (["heights", "slots", "contacts", "surveyMask", "offsets"] as const).map(
        (name) => storage[name] === litStorage[name],
      ),
      instances: storage["instances"]?.name,
      bound: Object.keys(storage).toSorted(),
      uniforms: (["bodyRotation", "cameraBodyFixedM", "atlas"] as const).map(
        (name) => caster.uniforms[name] === lit.uniforms[name],
      ),
      textures: caster.textures,
      indirect: caster.indirect,
      counted: (caster.instanceCount ?? 0) > 0,
    }).toEqual({
      material: `terrain-shadow-${made.pass.layout.vertexPath}`,
      mesh: true,
      shared: [true, true, true, true, true],
      instances: `terrain casters (${PRIMARY}) 1`,
      bound: (made.pass.layout.vertexPath === "baked-offsets"
        ? ["contacts", "heights", "instances", "offsets", "slots", "surveyMask"]
        : ["contacts", "heights", "instances", "slots", "surveyMask"]
      ).toSorted(),
      uniforms: [true, true, true],
      textures: {},
      indirect: undefined,
      counted: true,
    });
  });

  it("writes each cascade's casters as a byte-identical subset of the view's own records, in order", async () => {
    const made = await streamedAt(NEAR);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    const own = recordsIn(made.engine, `terrain instances (${PRIMARY})`);
    const cast = cascadesOf(frame).passes.flatMap((each) => {
      const draw = each.draws[0];
      if (draw === undefined || !draw.material.name.startsWith("terrain-shadow-")) {
        return [];
      }
      const records = recordsIn(
        made.engine,
        `terrain casters (${PRIMARY}) ${String(each.layer + 1)}`,
      );
      return [{ records, count: draw.instanceCount }];
    });
    expect(cast.length).toBeGreaterThan(0);
    for (const { records, count } of cast) {
      expect([records.length, subsequence(records, own)]).toEqual([count, true]);
    }
  });

  it("casts the interior patches' clusters through the cluster path, the edge patches' by the lit mesh (R14.T1.b)", async () => {
    const made = await clusteredPass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const cast = cascadesOf(frame);
    const dispatched = made.engine.dispatched.filter((d) => d.kernel === LIT_CULL);
    const interior = recordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    expect([
      frame.draws.map((draw) => draw.mesh.name),
      cast.clustered,
      cast.litMeshCasters,
      dispatched.map((d) => [d.pass, d.workgroups]),
      dispatched[0]?.bindings.buffers["instance_words"]?.name,
    ]).toEqual([
      // The interior patches drawn from the lit pass's buckets, a block's and a strip's (R14.T2.a).
      ["terrain clusters", "terrain clusters"],
      true,
      [0, 0, 0, 0],
      [[TERRAIN_CLUSTER_CULL_LABEL, [interior.length, 1, 1]]],
      `terrain interior instances (${PRIMARY})`,
    ]);
    // Dispatched before the caster passes are rendered, its buckets cleared before it.
    const render = made.engine.depthLayerRenders.at(-1);
    expect(render?.passes.length).toBe(4);
  });

  it("takes as each interior record's cull input the cascades castsInto admits its patch to, and its level's largest spacing (R14.T1.b)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(CLUSTERS_ALONE);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const interior = recordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    // The same frame by the way back casts the same records by the lit mesh.
    made.pass.setGeometry(GEOMETRY_BEFORE_R14);
    const back = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const byCascade = [1, 2, 3, 4].map((n) =>
      recordsIn(made.engine, `terrain casters (${PRIMARY}) ${String(n)}`),
    );
    const masks = interior.map((record) =>
      byCascade.reduce(
        (mask, records, k) => (records.includes(record) ? mask | (1 << k) : mask),
        0,
      ),
    );
    expect([inputs.length, interior.length > 0]).toEqual([2 * interior.length, true]);
    expect(interior.map((_, n) => inputs[2 * n])).toEqual(masks);
    expect(masks.some((mask) => mask !== 0)).toBe(true);
    const units = new Set(interior.map((_, n) => f32Of(inputs[2 * n + 1] ?? 0)));
    const levels = [...units].map((unit) => {
      for (let level = 0; level <= MAX_LEVEL; level += 1) {
        if (Math.fround(vertexSpacing(PLANET.figure.equatorialRadiusM, level).maxM) === unit) {
          return level;
        }
      }
      return -1;
    });
    expect(levels.filter((level) => level < 0)).toEqual([]);
    expect([
      cascadesOf(back).clustered,
      cascadesOf(back).litMeshCasters.some((n) => n > 0),
    ]).toEqual([false, true]);
  });

  it("draws each cascade's buckets after the lit mesh's casters, by the cluster caster at stride 1 (R14.T1.b)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(CLUSTERS_ALONE);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const pass = cascadesOf(frame).passes[2] ?? fail();
    const buckets = pass.draws.filter((draw) => draw.mesh.name === "terrain clusters");
    expect(
      buckets.map((draw) => [
        draw.material.name,
        draw.indirect?.buffer.name,
        draw.indirect?.offsetBytes,
        draw.storageBuffers?.["clusterLists"]?.name,
        draw.storageBuffers?.["instances"]?.name,
        draw.uniforms["clusterBase"]?.[0],
      ]),
    ).toEqual([
      [
        `terrain-cluster-shadow-${made.pass.layout.vertexPath}-still`,
        `terrain clusters (${PRIMARY}) arguments`,
        (2 * TEMPLATES + 0) * 20,
        `terrain clusters (${PRIMARY}) lists`,
        `terrain interior instances (${PRIMARY})`,
        2 * PASS_ENTRIES(made),
      ],
      [
        `terrain-cluster-shadow-${made.pass.layout.vertexPath}-still`,
        `terrain clusters (${PRIMARY}) arguments`,
        (2 * TEMPLATES + 4) * 20,
        `terrain clusters (${PRIMARY}) lists`,
        `terrain interior instances (${PRIMARY})`,
        2 * PASS_ENTRIES(made) + 4 * 64 * made.pass.layout.slots.slotCount,
      ],
    ]);
  });

  it("culls each cascade up to its stride cap at its texel, and draws every stride's buckets up to the cap (R14.T1.d)", async () => {
    const made = await clusteredPass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const dispatch = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL);
    const passes = parsePasses(dispatch?.bindings.uniforms["passes"] ?? new Uint32Array());
    const set = cascadesOf(frame).set;
    const buckets = (cascadesOf(frame).passes[1] ?? fail()).draws.filter(
      (draw) => draw.mesh.name === "terrain clusters",
    );
    expect([
      passes.map((pass) => pass.strideCap),
      passes.map((pass, k) => pass.toleranceM === Math.fround(set.cascades[k]?.texelM ?? 0)),
      buckets.map((draw) => draw.indirect?.offsetBytes),
    ]).toEqual([
      [8, 8, 8, 8],
      [true, true, true, true],
      [0, 1, 2, 3, 4, 5, 6, 7].map((t) => (1 * TEMPLATES + t) * 20),
    ]);
  });

  it("gives the overlay its depth on the cluster path: the edge draws, and the interior patches' clusters at stride 1 culled to the camera's frustum (R14.T1.e)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(OVERLAY_ALONE);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const dispatch = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL);
    const words = dispatch?.bindings.uniforms["passes"] ?? new Uint32Array();
    const passes = new DataView(words.buffer, words.byteOffset, words.byteLength);
    const params = dispatch?.bindings.uniforms["params"] ?? new Uint32Array();
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const interior = recordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    const overlay = frame.overlayDepth ?? fail();
    const at = 4 * CULL_PASS_BYTES;
    expect([
      [
        passes.getUint32(at, true),
        passes.getUint32(at + 4, true),
        passes.getFloat32(at + 20, true),
      ],
      new DataView(params.buffer, params.byteOffset).getUint32(60, true),
      interior.length > 0 && interior.every((_, n) => ((inputs[2 * n] ?? 0) & 0b1_0000) !== 0),
      overlay.edge.length === frame.draws.length - 1 &&
        overlay.edge.every((draw) => frame.draws.includes(draw)),
      overlay.clusters.map((draw) => [
        draw.material.name,
        draw.indirect?.offsetBytes,
        draw.storageBuffers?.["instances"]?.name,
      ]),
    ]).toEqual([
      [2, 0, 5],
      5,
      true,
      true,
      [
        [
          `terrain-cluster-depth-${made.pass.layout.vertexPath}-still`,
          (4 * TEMPLATES + 0) * 20,
          `terrain interior instances (${PRIMARY})`,
        ],
        [
          `terrain-cluster-depth-${made.pass.layout.vertexPath}-still`,
          (4 * TEMPLATES + 4) * 20,
          `terrain interior instances (${PRIMARY})`,
        ],
      ],
    ]);
  });

  it("culls the overlay's pass alone where no cascades are drawn, as on the low setting (R14.T1.e)", async () => {
    const made = await clusteredPass("low");
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    const dispatch = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL);
    const words = dispatch?.bindings.uniforms["passes"] ?? new Uint32Array();
    const passes = new DataView(words.buffer, words.byteOffset, words.byteLength);
    expect([
      frame.cascades,
      [0, 1, 2, 3, 4].map((k) => passes.getUint32(k * CULL_PASS_BYTES, true)),
      frame.overlayDepth?.clusters.length,
    ]).toEqual([null, [0, 0, 0, 0, 2], 2]);
  });

  it("redraws the overlay's depth by the way back and where its lever is redraw, culling no overlay pass (R14.T1.e)", async () => {
    const made = await clusteredPass("low");
    const culls = (): number => made.engine.dispatched.filter((d) => d.kernel === LIT_CULL).length;
    made.pass.setGeometry({ ...SETTINGS.low.geometry, overlayDepth: "redraw", litClusters: false });
    const before = culls();
    const redrawn = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    made.pass.setGeometry(GEOMETRY_BEFORE_R14);
    const back = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    expect([redrawn.overlayDepth, back.overlayDepth, culls() - before]).toEqual([null, null, 0]);
  });

  it("counts the overlay's triangles: its redraw's, or its edge draws' and its counted clusters' (R14.T1.e)", async () => {
    const made = await clusteredPass("low");
    made.pass.setGeometry({ ...SETTINGS.low.geometry, overlayDepth: "redraw", litClusters: false });
    made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    const redrawn = made.pass.counts(PRIMARY) ?? fail();
    made.pass.setGeometry(SETTINGS.low.geometry);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    expect([
      redrawn.overlayTriangles === redrawn.drawnTriangles,
      made.pass.counts(PRIMARY)?.overlayTriangles,
    ]).toEqual([
      true,
      // No counters' sample back unmeasured.
      null,
    ]);
  });

  it("casts by the lit mesh alone by the way back, and where no patch is interior (R14.T1.b)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(GEOMETRY_BEFORE_R14);
    const back = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const edge = await streamedAt(NEAR);
    // Its patches are edge bakes' stand-ins, which the edge stand-ins' lever takes onto the path
    // where their blocks are opaque (R14.T3.c's edge stand-ins): its way back keeps them off.
    edge.pass.setGeometry({ ...SETTINGS.high.geometry, edgeStandIns: "off" });
    const edgeFrame = edge.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([
      made.engine.dispatched.filter((d) => d.kernel === LIT_CULL).length,
      cascadesOf(back).clustered,
      edge.engine.dispatched.filter((d) => d.kernel === LIT_CULL).length,
      cascadesOf(edgeFrame).clustered,
    ]).toEqual([0, false, 0, false]);
  });

  it("refuses in its twin every cluster of a patch castsInto refuses, and keeps one only where its sphere meets the cascade's box (R14.T1.b)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(CLUSTERS_ALONE);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const dispatch = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL);
    const passes = parsePasses(dispatch?.bindings.uniforms["passes"] ?? new Uint32Array());
    const viewAxis = parsedViewAxis(dispatch?.bindings.uniforms["params"] ?? new Uint32Array());
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const instances = instanceOrigins(made.engine, `terrain interior instances (${PRIMARY})`);
    const records = clusterRecordsOf(made.engine);
    const twin = cullTwin(
      {
        instances: instances.map((instance, n) => ({
          ...instance,
          passMask: inputs[2 * n] ?? 0,
          unitM: f32Of(inputs[2 * n + 1] ?? 0),
        })),
        records: (page, cluster) => records(page, cluster),
        clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
        rotationColumns: [
          { x: 1, y: 0, z: 0 },
          { x: 0, y: 1, z: 0 },
          { x: 0, y: 0, z: 1 },
        ],
        passes,
        places: passes.map((_, k) => ({ bucketBase: 8 * k, listBase: 0 })),
        templates: 8,
        viewAxis,
      },
      0,
    );
    let entries = 0;
    for (const [bucket, list] of twin) {
      const k = Math.floor(bucket / 8);
      for (const entry of list) {
        const n = Math.floor(entry / 256);
        entries += 1;
        expect(((inputs[2 * n] ?? 0) >> k) & 1).toBe(1);
        // Stride 1 alone: a block's bucket 0, a strip's 4.
        expect([0, 4]).toContain(bucket % 8);
      }
    }
    expect(entries).toBeGreaterThan(0);
    // Every cluster of a patch admitted to a cascade and inside its box is kept.
    const kept = new Set([...twin.values()].flat().map(String));
    const refused: string[] = [];
    instances.forEach((instance, n) => {
      for (let k = 0; k < passes.length; k += 1) {
        if ((((inputs[2 * n] ?? 0) >> k) & 1) === 0) {
          continue;
        }
        const pass = passes[k] ?? fail();
        for (let cluster = 0; cluster < TERRAIN_CLUSTERS_PER_PAGE; cluster += 1) {
          const record = records(instance.page, cluster);
          const centre = add(instance.originM, record.centreM);
          if (
            cascadeKeeps(pass, centre, record.radiusM, 0) &&
            !kept.has(String(n * 256 + cluster))
          ) {
            refused.push(`${String(n)}:${String(cluster)} in ${String(k)}`);
          }
        }
      }
    });
    expect(refused).toEqual([]);
  });

  it("culls each cascade's casters by its receivers' depths: a patch's cascades a subset of castsInto's, some refused (R14.T1.c)", async () => {
    const made = await clusteredPass();
    /** Each record's cascades, interior then edge, as the frame's casters take them. */
    const cascadesNow = (): number[] => {
      made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
      const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
      const interior = recordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
      const edge = recordsIn(made.engine, `terrain instances (${PRIMARY})`);
      const byCascade = [1, 2, 3, 4].map((n) =>
        recordsIn(made.engine, `terrain casters (${PRIMARY}) ${String(n)}`),
      );
      // The cascades' bits alone: the fifth is the overlay's (plan R14, R14.T1.e).
      return [
        ...interior.map((_, n) => (inputs[2 * n] ?? 0) & 0b1111),
        ...edge.map((record) =>
          byCascade.reduce((m, records, k) => (records.includes(record) ? m | (1 << k) : m), 0),
        ),
      ];
    };
    const culled = cascadesNow();
    made.pass.setGeometry(CLUSTERS_ALONE);
    const boxed = cascadesNow();
    expect(culled).toHaveLength(boxed.length);
    expect(culled.filter((mask, n) => (mask & ~(boxed[n] ?? 0)) !== 0)).toEqual([]);
    expect(culled.some((mask, n) => mask !== boxed[n])).toBe(true);
  });

  it("packs each cascade's receivers into the cull's passes: the depths they sample it at and the cover below (R14.T1.c)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(EVERY_FRAME);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const dispatch = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL);
    const passes = parsePasses(dispatch?.bindings.uniforms["passes"] ?? new Uint32Array());
    const receivers = passes.map((pass) => pass.receivers ?? fail());
    const far = receivers.map((r) => r.depthM[1]);
    expect([
      receivers.length,
      receivers.map((r) => r.coveredBelow === null),
      far.every((d, k) => k === 0 || d > (far[k - 1] ?? Infinity)),
      // Each near end, the next nearer cascade's fade start, before that cascade's far end.
      receivers.every((r, k) => k === 0 || r.depthM[0] < (far[k - 1] ?? 0)),
      receivers.every((r) => r.reachM[0] > 0 && r.reachM[1] > 0),
    ]).toEqual([4, [true, false, false, false], true, true, true]);
    made.pass.setGeometry(CLUSTERS_ALONE);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const alone = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL);
    expect(
      parsePasses(alone?.bindings.uniforms["passes"] ?? new Uint32Array()).map((p) => p.receivers),
    ).toEqual([null, null, null, null]);
  });

  it("writes each slot's 116 cluster records at its upload, its fin clusters' too (R14.T1.g), and samples its counters only when measured (R14.T1.b)", async () => {
    const made = await clusteredPass();
    const writes = made.engine.writes.filter((w) => w.buffer === "terrain cluster records");
    const bytes = TERRAIN_PAGE_CLUSTERS * 32;
    expect(writes.length).toBeGreaterThan(0);
    expect(writes.every((w) => w.bytes === bytes && w.offsetBytes % bytes === 0)).toBe(true);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const copies = (): number =>
      made.engine.dispatched.filter((d) => d.kernel === "cluster stats reduction").length;
    const unmeasured = copies();
    made.pass.measureClusters(true);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([unmeasured, copies(), made.pass.clusterStats(PRIMARY)]).toEqual([0, 1, null]);
  });

  it("makes its cluster path again after a restore, its records, buckets and kernel new, its first frame's items a fresh pass's (R14.T1.b; Design note 16)", async () => {
    const shapeOf = (frame: TerrainViewFrame): unknown => ({
      draws: frame.draws.map(drawShape),
      casters: cascadesOf(frame).passes.map((pass) => pass.draws.map(drawShape)),
      overlay: frame.overlayDepth?.clusters.map(drawShape) ?? null,
    });
    const made = await clusteredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const made0 = {
      kernels: made.engine.kernelPairs.filter((pair) => pair.name === LIT_CULL).length,
      writes: made.engine.writes.length,
    };
    const created: string[] = [];
    const off = made.engine.onAllocation((event) => {
      if (event.kind === "created") {
        created.push(event.name);
      }
    });
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    stream(made, [passView(PRIMARY, NEAR)], 24);
    const restored = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    off();
    const fresh = await clusteredPass();
    const freshFrame = fresh.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([
      made.engine.kernelPairs.filter((pair) => pair.name === LIT_CULL).length > made0.kernels,
      made.engine.writes
        .slice(made0.writes)
        .some(
          (w) => w.buffer === "terrain cluster records" && w.bytes === TERRAIN_PAGE_CLUSTERS * 32,
        ),
      ["terrain cluster records", `terrain clusters (${PRIMARY}) lists`].every((name) =>
        created.includes(name),
      ),
    ]).toEqual([true, true, true]);
    expect(shapeOf(restored)).toEqual(shapeOf(freshFrame));
  });

  it("times one cull, the compaction's three dispatches and four caster passes a lit view-frame, one more for the counters while measured, and resolves nothing of its own (R14.T1.b, T2.a; Design note 18)", async () => {
    // The heaviest frame's three views (a photorealistic primary and two instruments) of one body.
    const made = await clusteredPass();
    const views = [
      passView(PRIMARY, NEAR),
      passView(INSTRUMENT, viewOver(50, 40)),
      passView(instrumentViewId(2), viewOver(50, 80)),
    ];
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    stream(made, views, 24);
    const timedOf = (measuring: boolean): readonly [number, number, number] => {
      made.pass.measureClusters(measuring);
      const dispatched = made.engine.dispatched.length;
      const layered = made.engine.depthLayerRenders.length;
      const frames = made.engine.targetFrames.length;
      for (const view of views) {
        made.pass.drawFor(view.id, frameOf(view.view, true));
      }
      const passes =
        made.engine.dispatched.slice(dispatched).filter((d) => d.pass !== undefined).length +
        made.engine.depthLayerRenders
          .slice(layered)
          .reduce((sum, render) => sum + render.passes.length, 0);
      return [
        passes,
        made.engine.targetFrames.length - frames,
        made.engine.dispatched.length - dispatched,
      ];
    };
    // Unmeasured: 3 × (the cull, the compaction's count, scan and scatter, and four cascades);
    // measured, each view's reduction too. Each rides the view's next render's resolve.
    expect([timedOf(false), timedOf(true)]).toEqual([
      [24, 0, 12],
      [27, 0, 15],
    ]);
  });

  it("falls back to today's path where its cull kernel is refused, and states the refusal (R14.T1.f; Design note 16)", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const broken = { ...CLUSTER_CULL_KERNEL, name: "broken cluster cull" };
    const compile = engine.createComputeAsync.bind(engine);
    vi.spyOn(engine, "createComputeAsync").mockImplementation((pair) =>
      pair === broken ? Promise.reject(new Error("the WGSL does not compile")) : compile(pair),
    );
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const made = await litPass("high", engine, () => 8_000, { clusterCullKernel: broken });
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    stream(made, [passView(PRIMARY, NEAR)], 24);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const cast = cascadesOf(frame);
    expect([
      made.pass.clusterRefusal,
      cast.clustered,
      cast.litMeshCasters.some((count) => count > 0),
      frame.overlayDepth,
      made.engine.dispatched.filter((d) => d.pass === TERRAIN_CLUSTER_CULL_LABEL).length,
    ]).toEqual(["broken cluster cull refused: the WGSL does not compile", false, true, null, 0]);
    // A pass whose kernel compiles states nothing.
    expect((await clusteredPass()).pass.clusterRefusal).toBeNull();
  });

  it("draws the interior patches from the lit pass's buckets at stride 1, by the lit cluster form, before the edge patches' draw (R14.T2.a)", async () => {
    const made = await clusteredPass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const lists = `terrain clusters (${PRIMARY}) lists`;
    const pass = 5;
    const entries = PASS_ENTRIES(made);
    const blocks = made.pass.layout.slots.slotCount * 4 * 64;
    expect(
      frame.draws.map((draw) => [
        draw.mesh.name,
        draw.material.name,
        draw.indirect?.offsetBytes,
        [...(draw.uniforms["clusterBase"] ?? [])],
        draw.storageBuffers?.["instances"]?.name,
        draw.storageBuffers?.["clusterLists"]?.name,
      ]),
    ).toEqual([
      [
        "terrain clusters",
        `terrain-lit-cluster-${path}-still`,
        (TEMPLATES * pass + 0) * 20,
        [pass * entries],
        `terrain interior instances (${PRIMARY})`,
        lists,
      ],
      [
        "terrain clusters",
        `terrain-lit-cluster-${path}-still`,
        (TEMPLATES * pass + 4) * 20,
        [pass * entries + blocks],
        `terrain interior instances (${PRIMARY})`,
        lists,
      ],
    ]);
    // The lit draw's own uniforms, the same arrays, which each frame rewrites in place.
    expect(frame.draws[0]?.uniforms["cascadeViewAxis"]).toBe(
      made.pass.drawFor(PRIMARY, frameOf(NEAR, true)).draws[0]?.uniforms["cascadeViewAxis"],
    );
  });

  it("culls the lit pass as an ordered camera pass with the body's horizon, then compacts it: the count, the scan and the scatter (R14.T2.a)", async () => {
    const made = await clusteredPass();
    const from = made.engine.dispatched.length;
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const dispatched = made.engine.dispatched.slice(from);
    const interior = recordsIn(made.engine, `terrain interior instances (${PRIMARY})`).length;
    const cull = dispatched.find((d) => d.kernel === LIT_CULL) ?? fail();
    const lit = parseLitPass(cull.bindings.uniforms["passes"] ?? new Uint32Array(), 5);
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const compactions = dispatched.filter((d) => d.kernel === "cluster compaction");
    expect([
      dispatched.map((d) => d.kernel).slice(0, 4),
      compactions.map((d) => [
        d.pass,
        d.workgroups,
        d.bindings.uniforms["params"]?.[3],
        Object.keys(d.bindings.buffers).toSorted(),
      ]),
      Object.keys(cull.bindings.buffers).includes("marks"),
      [lit.planes.length, lit.strideCap, lit.horizon !== null],
      Array.from({ length: interior }, (_, n) => ((inputs[2 * n] ?? 0) >> 5) & 1),
    ]).toEqual([
      [LIT_CULL, "cluster compaction", "cluster compaction", "cluster compaction"],
      [0, 1, 2].map((phase) => [
        TERRAIN_CLUSTER_COMPACT_LABEL,
        phase === 1 ? [1, 1, 1] : [interior, 1, 1],
        phase,
        ["args", "lists", "marks", "tallies"],
      ]),
      true,
      [5, 1, true],
      Array.from({ length: interior }, () => 1),
    ]);
    // The horizon is R05's occluder's seen from the camera: its cone's sine R_occ ÷ |C|.
    const camera = NEAR.cameraM;
    const occluder = horizonCone(PLANET, camera).occluderRadiusM;
    expect(lit.horizon?.sin).toBe(Math.fround(occluder / norm(camera)));
  });

  it("marks in its twin every cluster the lit pass's frustum and horizon keep, and refuses the others, in the draw set's order after the compaction (R14.T2.a)", async () => {
    const made = await clusteredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const cull = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL) ?? fail();
    const lit = parseLitPass(cull.bindings.uniforms["passes"] ?? new Uint32Array(), 5);
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const instances = instanceOrigins(made.engine, `terrain interior instances (${PRIMARY})`);
    const records = clusterRecordsOf(made.engine);
    const input = {
      instances: instances.map((instance, n) => ({
        ...instance,
        passMask: inputs[2 * n] ?? 0,
        unitM: f32Of(inputs[2 * n + 1] ?? 0),
      })),
      records: (page: number, cluster: number) => records(page, cluster),
      clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
      rotationColumns: [
        { x: 1, y: 0, z: 0 },
        { x: 0, y: 1, z: 0 },
        { x: 0, y: 0, z: 1 },
      ] as const,
      passes: [null, null, null, null, null, lit],
      places: Array.from({ length: 6 }, (_, k) => ({ bucketBase: 8 * k, listBase: 0 })),
      templates: 8,
      viewAxis: { x: 0, y: 0, z: -1 },
    };
    const marks = cullMarksTwin(input, 0);
    const kept = [...marks].filter((mark) => mark !== NO_MARK);
    // Stride 1 alone: a block's template 0 and a strip's 4; some clusters of the view's patches
    // are refused, behind it or past its sides.
    expect(new Set(kept)).toEqual(new Set([0, 4]));
    expect([kept.length > 0, kept.length < marks.length]).toEqual([true, true]);
    const { lists } = compactTwin(marks, instances.length, TERRAIN_CLUSTERS_PER_PAGE, 8);
    for (const list of lists) {
      expect(list.toSorted((a, b) => a - b)).toEqual(list);
    }
  });

  it("draws its interior patches by the interior mesh where the lit cluster form is refused, and states the refusal (R14.T2.a)", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    engine.failMaterials((name) => name.startsWith("terrain-lit-cluster"));
    const made = await litPass("high", engine);
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    stream(made, [passView(PRIMARY, NEAR)], 24);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([
      frame.draws.map((draw) => draw.mesh.name),
      made.pass.clusterRefusal?.startsWith("the lit-cluster form refused"),
      made.engine.dispatched.some((d) => d.kernel === "cluster compaction"),
    ]).toEqual([["terrain lit interior patch"], true, false]);
  });

  it("gives two views of one pass each their own four passes, of their own records", async () => {
    const made = await litPass();
    const primary = viewOver(50, 0);
    const instrument = viewOver(80, 90);
    stream(made, [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)]);
    const first = made.pass.drawFor(PRIMARY, frameOf(primary, true));
    const second = made.pass.drawFor(INSTRUMENT, frameOf(instrument, true));
    const renders = made.engine.depthLayerRenders;
    const instancesOf = (frame: TerrainViewFrame): string | undefined =>
      terrainCaster(cascadesOf(frame).passes[0] ?? fail()).storageBuffers?.["instances"]?.name;
    expect([
      renders.map(({ owner, passes }) => [owner, passes.length]),
      instancesOf(first),
      instancesOf(second),
      subsequence(
        recordsIn(made.engine, `terrain casters (${INSTRUMENT}) 1`),
        recordsIn(made.engine, `terrain instances (${INSTRUMENT})`),
      ),
    ]).toEqual([
      [
        [PRIMARY, 4],
        [INSTRUMENT, 4],
      ],
      `terrain casters (${PRIMARY}) 1`,
      `terrain casters (${INSTRUMENT}) 1`,
      true,
    ]);
  });

  it("releases a leaving view's caster buffers with its own", async () => {
    const made = await litPass();
    const primary = viewOver(50, 0);
    const instrument = viewOver(80, 90);
    stream(made, [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)]);
    made.pass.drawFor(INSTRUMENT, frameOf(instrument, true));
    const destroyed: string[] = [];
    made.engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    made.pass.step({ views: [passView(PRIMARY, primary)], grounded: [] });
    expect(destroyed.filter((name) => name.startsWith("terrain casters")).toSorted()).toEqual(
      [1, 2, 3, 4].map((n) => `terrain casters (${INSTRUMENT}) ${String(n)}`),
    );
  });

  it("appends the other casters' draws after the terrain's in each pass, until they are removed", async () => {
    const made = await streamedAt(NEAR);
    made.pass.setGeometry(EVERY_FRAME);
    const asked: Array<readonly [number, string]> = [];
    const other: DrawItem = {
      mesh: { kind: "mesh", name: "craft" } satisfies MeshHandle,
      material: { kind: "material", name: "craft caster" } satisfies MaterialHandle,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    const remove = made.shadows.casters.add((_set, index, view) => {
      asked.push([index, view]);
      return [other];
    });
    const view = viewOver(50);
    const withOther = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(view, true))).passes;
    remove();
    const without = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(view, true))).passes;
    expect([
      asked,
      withOther.map((each) => each.draws.at(-1) === other),
      withOther.map((each) => each.draws.length - 1),
      without.map((each) => each.draws.length),
    ]).toEqual([
      [0, 1, 2, 3].map((index) => [index, PRIMARY]),
      [true, true, true, true],
      without.map((each) => each.draws.length),
      without.map((each) => each.draws.length),
    ]);
  });

  it("skips every pass at w = 0, far out, and still fits", async () => {
    const made = await streamedAt(ORBIT);
    const frame = made.pass.drawFor(PRIMARY, frameOf(ORBIT, true));
    expect([
      cascadesOf(frame).set.weight,
      cascadesOf(frame).passes,
      made.engine.depthLayerRenders.length,
      made.engine.counts.depthLayerTargets,
      drawOf(frame).uniforms["cascadeViewAxis"]?.[3],
    ]).toEqual([0, [], 0, 0, 0]);
  });

  it("fits nothing for a body that takes no cascades, or under no star", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const unlit = made.pass.drawFor(PRIMARY, {
      ...frameOf(view, true),
      lighting: { ...STARLIT, lights: [] },
    });
    const other = made.pass.drawFor(PRIMARY, frameOf(view, false));
    expect([unlit.cascades, other.cascades, made.engine.depthLayerRenders.length]).toEqual([
      null,
      null,
      0,
    ]);
  });
});

/** The draws of a frame, or a pass, as (mesh, material, bucket, instances). */
function named(draws: ReadonlyArray<DrawItem>): unknown[] {
  return draws.map((draw) => [
    draw.mesh.name,
    draw.material.name,
    draw.indirect === undefined ? null : draw.indirect.offsetBytes / 20,
    draw.storageBuffers?.["instances"]?.name,
  ]);
}

/** Of a frame's cull inputs, whether each record's is kept whole (R10.T16.i's fades). */
function keptWhole(words: ReadonlyArray<number>): boolean[] {
  return words.filter((_, k) => k % 2 === 0).map((word) => (word & CULL_INPUT_WHOLE) !== 0);
}

/** `made` stepped nearer with nothing baked: the finer patches asked for drawn by their ancestors. */
function nearerUnbaked(made: Made): void {
  const nearer = viewOver(10);
  made.pass.step({ views: [passView(PRIMARY, nearer)], grounded: [] });
  made.pass.drawFor(PRIMARY, frameOf(nearer, true));
}

/** The stand-ins of `made`'s last draw set. */
function standInsOf(made: Made): number {
  return (made.pass.lastStep?.drawSet.patches ?? [])
    .slice(0, made.pass.lastStep?.drawSet.count ?? 0)
    .filter((entry) => entry.standIn).length;
}

/** Whether a sub-patch word's area lies on the half x < 32 of its slot's patch. */
function onSurveyedHalf(subWord: number): boolean {
  const quads = 64 / 2 ** (subWord & 31);
  return ((subWord >>> 5) & 0x1fff) * quads + quads <= 32;
}

describe("the edge patches on the cluster path (plan R14, R14.T1.g)", () => {
  /**
   * A body's layers surveyed on the patch's half x < 32 alone, of a field 20 levels deep: its patches
   * edge patches at every cell code but 0's and 6's.
   */
  function halfSurveyed(): TerrainLayers {
    const surveyMask = new Uint8Array(65 * 65);
    for (let y = 0; y <= 64; y += 1) {
      surveyMask.fill(255, 65 * y, 65 * y + 32);
    }
    return { ...layers(), surveyMask, fieldLevel: 20 };
  }

  /**
   * A body's layers surveyed left of x = 40 alone, of a field 20 levels deep: each patch's blocks of
   * the columns 0 to 3 opaque at every cell code but 6's, so that a stand-in one level down lies on
   * opaque blocks over x < 32 and across the survey's edge over x > 32 (R14.T3.c's edge stand-ins).
   */
  function surveyedLeftOf40(): TerrainLayers {
    const surveyMask = new Uint8Array(65 * 65);
    for (let y = 0; y <= 64; y += 1) {
      surveyMask.fill(255, 65 * y, 65 * y + 40);
    }
    return { ...layers(), surveyMask, fieldLevel: 20 };
  }

  /** A streamed pass at {@link NEAR} over {@link halfSurveyed}'s layers, its fades cut. */
  async function edgePass(
    given?: CountingRenderEngine,
    reducedMotion = true,
    setting: "high" | "low" = "high",
    layersFor: () => TerrainLayers = halfSurveyed,
  ): Promise<Made> {
    const made = await litPass(setting, given);
    made.pass.setGeometry(
      setting === "high" ? HIGH_AT_STRIDE_1 : { ...SETTINGS.low.geometry, strides: "off" },
    );
    made.pass.setReducedMotion(reducedMotion);
    made.lane().layers = layersFor();
    // Until nothing more is asked for, so that no patch is drawn by a stand-in.
    stream(made, [passView(PRIMARY, NEAR)], 24);
    return made;
  }

  it("draws them with the interior patches from the lit pass's buckets, the may-discard ones by the fin form, and none by the lit mesh", async () => {
    const made = await edgePass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const instances = `terrain interior instances (${PRIMARY})`;
    const lit = 5 * TEMPLATES;
    expect([frame.patches.fins > 0, frame.patches.fins === frame.patches.edge]).toEqual([
      true,
      true,
    ]);
    expect(named(frame.draws)).toEqual([
      ["terrain clusters", `terrain-lit-cluster-${path}-still`, lit, instances],
      ["terrain clusters", `terrain-lit-cluster-${path}-still`, lit + 4, instances],
      ["terrain clusters", `terrain-lit-fin-${path}-still`, lit + 8, instances],
      ["terrain clusters", `terrain-lit-fin-${path}-still`, lit + 9, instances],
      ["terrain clusters", `terrain-lit-fin-${path}-still`, lit + 10, instances],
      ["terrain clusters", `terrain-lit-fin-${path}-still`, lit + 11, instances],
    ]);
    // Counted as edge patches on the cluster path, their drawn triangles their lit mesh's.
    const counts = made.pass.counts(PRIMARY);
    expect([counts?.patches, counts?.drawnTriangles]).toEqual([
      { interior: 0, edge: frame.patches.edge, fins: frame.patches.edge, edgeStandIns: 0 },
      (frame.patches.edge * LIT_PATCH_INDICES) / 3,
    ]);
    // Their records in the interior patches' buffer, none in the lit mesh's.
    expect([
      recordsIn(made.engine, instances).length,
      recordsIn(made.engine, `terrain instances (${PRIMARY})`).length,
    ]).toEqual([made.pass.lastStep?.drawSet.count ?? -1, 0]);
  });

  it("draws their may-discard buckets forward after the visibility buffer's resolve, by the fin form, and rasterises none (R14.T2.c)", async () => {
    const made = await edgePass();
    made.pass.setGeometry({ ...HIGH_AT_STRIDE_1, shading: "visibility" });
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    await settle();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const instances = `terrain interior instances (${PRIMARY})`;
    const lit = 5 * TEMPLATES;
    expect([named(frame.visibility?.raster ?? []), named(frame.visibility?.forward ?? [])]).toEqual(
      [
        [
          ["terrain cluster ids", `terrain-id-cluster-${path}-still`, lit, instances],
          ["terrain cluster ids", `terrain-id-cluster-${path}-still`, lit + 4, instances],
        ],
        [8, 9, 10, 11].map((t) => [
          "terrain clusters",
          `terrain-lit-fin-${path}-still`,
          lit + t,
          instances,
        ]),
      ],
    );
  });

  it("casts them at stride 1 through the cull, the may-discard buckets by the fin caster", async () => {
    const made = await edgePass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const units = new Set(
      Array.from({ length: made.pass.lastStep?.drawSet.count ?? -1 }, (_, n) =>
        f32Of(inputs[2 * n + 1] ?? 0),
      ),
    );
    expect([...units]).toEqual([Math.fround(1e30)]);
    const cascade = cascadesOf(frame).passes[1] ?? fail();
    const clusters = cascade.draws.filter((draw) => draw.mesh.name === "terrain clusters");
    expect(
      clusters.map((draw) => [draw.material.name, (draw.indirect?.offsetBytes ?? 0) / 20]),
    ).toEqual([
      ...[0, 1, 2, 3, 4, 5, 6, 7].map((t) => [
        `terrain-cluster-shadow-${path}-still`,
        TEMPLATES + t,
      ]),
      ...[8, 9, 10, 11].map((t) => [`terrain-fin-shadow-${path}-still`, TEMPLATES + t]),
    ]);
    expect(cascadesOf(frame).litMeshCasters).toEqual([0, 0, 0, 0]);
  });

  it("draws a stand-in of an interior bake on the cluster path", async () => {
    const made = await clusteredPass();
    nearerUnbaked(made);
    expect([
      standInsOf(made) > 0,
      recordsIn(made.engine, `terrain instances (${PRIMARY})`),
    ]).toEqual([true, []]);
  });

  /** The sub-patch instances of `made`'s interior records: their words, and whether kept whole. */
  function subPatchesOf(made: Made): Array<{
    readonly levels: number;
    readonly masked: boolean;
    readonly kept: boolean;
    readonly holdsAndFades: number;
  }> {
    const per = INSTANCE_RECORD_BYTES / 4;
    const records = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    const whole = keptWhole(wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`));
    return whole.flatMap((kept, k) => {
      const word = records[k * per + INSTANCE_SUB_PATCH_WORD] ?? 0;
      const holds = (records[k * per + 7] ?? 0) >>> 2;
      const fades = (records[k * per + 9] ?? 0) | (records[k * per + 10] ?? 0);
      return word === 0
        ? []
        : [
            {
              levels: word & 31,
              masked: (word & TERRAIN_SUB_PATCH_MASK) !== 0,
              kept,
              holdsAndFades: (holds & 0xffff) | fades,
            },
          ];
    });
  }

  it("draws a sub-patch of an interior bake one to three levels down as a mask of its slot's patch's own blocks, culled by their spheres, with none of its area's holds or edges' fades (Design note 19)", async () => {
    const made = await clusteredPass();
    nearerUnbaked(made);
    const masks = subPatchesOf(made).filter(({ levels }) => levels <= TERRAIN_MASK_LEVELS);
    expect([
      masks.length > 0,
      masks.every(({ masked, kept, holdsAndFades }) => masked && !kept && holdsAndFades === 0),
    ]).toEqual([true, true]);
  });

  it("draws a stand-in of an edge bake by the lit mesh where the edge stand-ins are off, the way back, its cull inputs without their bits", async () => {
    const made = await edgePass();
    made.pass.setGeometry({ ...HIGH_AT_STRIDE_1, edgeStandIns: "off" });
    nearerUnbaked(made);
    const standIns = standInsOf(made);
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    expect([
      standIns > 0,
      recordsIn(made.engine, `terrain instances (${PRIMARY})`).length,
      inputs.filter((word, k) => k % 2 === 0 && (word & TERRAIN_INPUT_STAND_IN_BITS) !== 0),
      made.pass.counts(PRIMARY)?.patches.edgeStandIns,
    ]).toEqual([true, standIns, [], 0]);
  });

  /** Each interior record's sub-patch word, cull input and error unit, of `made`'s last frame. */
  function interiorInputsOf(made: Made): Array<{
    readonly subWord: number;
    readonly bits: number;
    readonly unitM: number;
  }> {
    const per = INSTANCE_RECORD_BYTES / 4;
    const records = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    return Array.from({ length: records.length / per }, (_, k) => ({
      subWord: records[k * per + INSTANCE_SUB_PATCH_WORD] ?? 0,
      bits: (inputs[2 * k] ?? 0) & TERRAIN_INPUT_STAND_IN_BITS,
      unitM: f32Of(inputs[2 * k + 1] ?? 0),
    }));
  }

  it("draws an edge bake's stand-ins on the cluster path where they are masks: on opaque blocks as an interior bake's, one level down across the survey's edge as an edge mask, the rest by the lit mesh, counted apart from the fins (R14.T3.c's edge stand-ins)", async () => {
    const made = await edgePass(undefined, true, "high", surveyedLeftOf40);
    nearerUnbaked(made);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(10), true));
    const subs = interiorInputsOf(made).filter(({ subWord }) => subWord !== 0);
    const surveyed = subs.filter(({ bits }) => (bits & TERRAIN_INPUT_SURVEYED) !== 0);
    const masks = subs.filter(({ bits }) => (bits & TERRAIN_INPUT_EDGE_MASK) !== 0);
    const litMesh = recordsIn(made.engine, `terrain instances (${PRIMARY})`).length;
    expect([
      surveyed.length > 0,
      masks.length > 0,
      surveyed.every(({ subWord }) => onSurveyedHalf(subWord)),
      masks.every(
        ({ subWord }) => (subWord & 31) === 1 && (subWord & TERRAIN_SUB_PATCH_MASK) !== 0,
      ),
      subs.every(({ bits }) => bits !== 0),
      frame.patches.edgeStandIns,
      frame.patches.fins + frame.patches.edgeStandIns + litMesh,
      frame.patches.edge,
    ]).toEqual([
      true,
      true,
      true,
      true,
      true,
      subs.length,
      frame.patches.edge,
      (made.pass.lastStep?.drawSet.count ?? -1) - frame.patches.interior,
    ]);
    // Of one slot's sibling edge masks, the one of x 1 leaves the column's side run to the one of
    // x 0, and the one of y 1 the row's to the one of y 0.
    const shared = masks.filter(
      ({ subWord }) => ((subWord >>> 5) & 1) === 0 && ((subWord >>> 18) & 1) === 0,
    );
    expect(
      masks.every(
        ({ subWord, bits }) =>
          ((subWord >>> 5) & 1) === 1 || (bits & TERRAIN_INPUT_COLUMN_RUN_SHARED) === 0,
      ),
    ).toBe(true);
    expect(
      shared.every(
        ({ bits }) =>
          (bits & (TERRAIN_INPUT_COLUMN_RUN_SHARED | TERRAIN_INPUT_ROW_RUN_SHARED)) === 0,
      ),
    ).toBe(true);
  });

  it("casts an edge bake's stand-in on opaque blocks as an interior bake's, by its level's spacing, and an edge mask at stride 1, as an edge patch (R14.T3.c's edge stand-ins)", async () => {
    const made = await edgePass(undefined, true, "high", surveyedLeftOf40);
    nearerUnbaked(made);
    const units = interiorInputsOf(made)
      .filter(({ subWord }) => subWord !== 0)
      .map(({ bits, unitM }) => [
        (bits & TERRAIN_INPUT_SURVEYED) !== 0,
        unitM === Math.fround(1e30),
      ]);
    expect([...new Set(units.map((pair) => pair.join()))].toSorted()).toEqual([
      "false,true",
      "true,false",
    ]);
  });

  it("draws low's edge stand-ins on opaque blocks on their own grids, kept whole, and no mask, where they are surveyed (R14.T3.c's edge stand-ins)", async () => {
    const made = await edgePass(undefined, true, "low", surveyedLeftOf40);
    nearerUnbaked(made);
    const subs = interiorInputsOf(made).filter(({ subWord }) => subWord !== 0);
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const whole = keptWhole(inputs);
    const per = INSTANCE_RECORD_BYTES / 4;
    const records = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    const onPath = Array.from({ length: records.length / per }, (_, k) => k).filter(
      (k) => (records[k * per + INSTANCE_SUB_PATCH_WORD] ?? 0) !== 0,
    );
    expect([
      subs.length > 0,
      subs.every(({ bits }) => bits === TERRAIN_INPUT_SURVEYED),
      subs.every(
        ({ subWord }) => (subWord & TERRAIN_SUB_PATCH_MASK) === 0 && onSurveyedHalf(subWord),
      ),
      onPath.every((k) => whole[k] === true),
    ]).toEqual([true, true, true, true]);
  });

  it("reuses their casters, the fin caster's among them, on a still camera, and renders them once again where the fins' lever turns (R14.T6.a)", async () => {
    const made = await edgePass();
    made.pass.setGeometry(EVERY_FRAME);
    const still = frameOf(NEAR, true);
    const first = cascadesOf(made.pass.drawFor(PRIMARY, still));
    const finned = first.passes.some((pass) =>
      pass.draws.some((draw) => draw.material.name.startsWith("terrain-fin-shadow")),
    );
    const layersNow = (): number[] => layersOf(made.pass.drawFor(PRIMARY, still));
    const again = layersNow();
    made.pass.setGeometry({ ...EVERY_FRAME, finClusters: false });
    expect([finned, first.passes.length, again, layersNow(), layersNow()]).toEqual([
      true,
      4,
      [],
      [0, 1, 2, 3],
      [],
    ]);
  });

  it("gives the overlay its depth of them on the cluster path, the may-discard buckets by the fin depth", async () => {
    const made = await edgePass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const overlay = frame.overlayDepth ?? fail();
    const at = 4 * TEMPLATES;
    expect([
      overlay.edge,
      overlay.clusters.map((d) => [d.material.name, (d.indirect?.offsetBytes ?? 0) / 20]),
    ]).toEqual([
      [],
      [
        [`terrain-cluster-depth-${path}-still`, at],
        [`terrain-cluster-depth-${path}-still`, at + 4],
        [`terrain-fin-depth-${path}-still`, at + 8],
        [`terrain-fin-depth-${path}-still`, at + 9],
        [`terrain-fin-depth-${path}-still`, at + 10],
        [`terrain-fin-depth-${path}-still`, at + 11],
      ],
    ]);
  });

  it("draws them by the lit mesh by the way back, the interior patches' path as before", async () => {
    const made = await edgePass();
    made.pass.setGeometry({ ...SETTINGS.high.geometry, finClusters: false });
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([
      frame.patches.fins,
      frame.draws.map((draw) => draw.mesh.name),
      recordsIn(made.engine, `terrain instances (${PRIMARY})`).length,
    ]).toEqual([0, ["terrain lit patch"], made.pass.lastStep?.drawSet.count ?? -1]);
  });

  it("draws them by the lit mesh where their fin form is refused, and states the refusal", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    engine.failMaterials((name) => name.startsWith("terrain-lit-fin"));
    const made = await edgePass(engine);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([
      frame.patches.fins,
      frame.draws.map((draw) => draw.mesh.name),
      made.pass.clusterRefusal?.startsWith("the lit-fin form refused"),
    ]).toEqual([0, ["terrain lit patch"], true]);
  });

  it("keeps whole in the cull each record whose heights fade from another surface, as a patch rising from the datum's (R10.T16.i)", async () => {
    const rising = await edgePass(undefined, false);
    rising.pass.drawFor(PRIMARY, frameOf(NEAR, true, 0));
    const fading = wordsIn(rising.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const cut = await edgePass();
    cut.pass.drawFor(PRIMARY, frameOf(NEAR, true, 0));
    const still = wordsIn(cut.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    expect([keptWhole(fading).some(Boolean), keptWhole(still).some(Boolean)]).toEqual([
      true,
      false,
    ]);
  });

  it("makes its fin forms again after a restore, its first frame's items a fresh pass's (Design note 16)", async () => {
    const shapeOf = (frame: TerrainViewFrame): unknown => ({
      draws: frame.draws.map(drawShape),
      casters: cascadesOf(frame).passes.map((pass) => pass.draws.map(drawShape)),
      overlay: frame.overlayDepth?.clusters.map(drawShape) ?? null,
    });
    const made = await edgePass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    made.lane().layers = halfSurveyed();
    stream(made, [passView(PRIMARY, NEAR)], 24);
    const restored = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const fresh = await edgePass();
    const freshFrame = fresh.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect(restored.patches.fins).toBeGreaterThan(0);
    expect(shapeOf(restored)).toEqual(shapeOf(freshFrame));
  });
});

/**
 * An instance record's strides (plan R14, R14.T3.c): its scale, its edges' shared bands as the
 * shaders read them back, m, at `edgeBandIndex`, and its quadrant word.
 */
interface RecordStrides {
  readonly strideScale: StrideScale;
  readonly edges: number[];
  readonly quadrant: number;
}

/** Whether a record has no strides: no scale, and none at any edge. */
function noStrides({ strideScale, edges }: RecordStrides): boolean {
  return strideScale.perErrorM === 0 && edges.every((band) => band === NO_STRIDE_BAND);
}

/** Each interior record's draw set level, in the records' order. */
function levelsOf(made: Made): number[] {
  return (made.pass.lastStep?.drawSet.patches ?? [])
    .slice(0, made.pass.lastStep?.drawSet.count ?? 0)
    .map((drawn) => drawn.patch.key.level);
}

describe("the camera's strides on the cluster path (plan R14, R14.T3.c)", () => {
  /** e₁ to e₈ of every cluster: 0, 1, 2 and 4 cm, smooth ground. */
  const ERRORS = (() => {
    const out = new Float16Array(4 * TERRAIN_CLUSTERS_PER_PAGE);
    for (let c = 0; c < TERRAIN_CLUSTERS_PER_PAGE; c += 1) {
      out.set([0, 0.01, 0.02, 0.04], 4 * c);
    }
    return out;
  })();

  const STRIDED: GeometrySettings = { ...SETTINGS.high.geometry, strides: "measured" };
  /** The strides under the patches' budget alone (R14.T3.e). */
  const PATCHED: GeometrySettings = { ...STRIDED, terrainBudget: "patches" };

  /**
   * A clustered pass of `setting`, high's by default, whose bakes carry `errors`, streamed with
   * `grounded`'s contacts.
   */
  async function measuredPass(
    errors: Float16Array | null = ERRORS,
    grounded: ReadonlyArray<GroundContact> = [],
    setting: "high" | "low" = "high",
  ): Promise<Made> {
    const made = await litPass(setting);
    made.pass.setReducedMotion(true);
    made.pass.setGeometry(setting === "high" ? STRIDED : SETTINGS.low.geometry);
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    made.lane().clusterErrors = errors;
    for (let n = 0; n < 24; n += 1) {
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded });
      made.lane().bakeDemand(made.pass);
    }
    made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded });
    return made;
  }

  /** Each interior record's strides as last written. */
  function stridesOf(made: Made): RecordStrides[] {
    const words = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    const per = INSTANCE_RECORD_BYTES / 4;
    return Array.from({ length: words.length / per }, (_record, n) => ({
      strideScale: {
        statedM: f32Of(words[per * n + 12] ?? 0),
        perErrorM: f32Of(words[per * n + 13] ?? 0),
      },
      edges: Array.from({ length: EDGE_BANDS }, (_half, h) =>
        edgeBandMOfHalf(
          ((words[per * n + INSTANCE_EDGE_BANDS_WORD + (h >> 1)] ?? 0) >>> (16 * (h & 1))) & 0xffff,
        ),
      ),
      quadrant: words[per * n + 7] ?? 0,
    }));
  }

  it("gives a sub-patch stand-in on the cluster path no bands where the stand-ins' strides are off, so that its camera passes draw it at stride 1", async () => {
    const made = await measuredPass();
    made.pass.setGeometry({ ...STRIDED, standInStrides: "off" });
    nearerUnbaked(made);
    const per = INSTANCE_RECORD_BYTES / 4;
    const words = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    const records = stridesOf(made).map((strides, k) => ({
      none: noStrides(strides),
      subPatch: (words[k * per + INSTANCE_SUB_PATCH_WORD] ?? 0) !== 0,
    }));
    const subPatches = records.filter(({ subPatch }) => subPatch);
    expect([
      subPatches.length > 0,
      subPatches.every(({ none }) => none),
      records.some(({ subPatch, none }) => !subPatch && !none),
    ]).toEqual([true, true, true]);
  });

  it("gives each stand-in's mask its slot's patch's scale where the stand-ins' strides are all, its holds' shares none, and none where they are own-grids or off, and marks none for the camera passes' cap (R14.T3.c's stand-ins)", async () => {
    const masksOf = async (
      standInStrides: GeometrySettings["standInStrides"],
    ): Promise<
      Array<{ readonly scaled: boolean; readonly marked: boolean; readonly shares: number }>
    > => {
      const made = await measuredPass();
      made.pass.setGeometry({ ...STRIDED, standInStrides });
      nearerUnbaked(made);
      const per = INSTANCE_RECORD_BYTES / 4;
      const words = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
      return stridesOf(made).flatMap(({ strideScale, quadrant }, k) =>
        ((words[k * per + INSTANCE_SUB_PATCH_WORD] ?? 0) & TERRAIN_SUB_PATCH_MASK) === 0
          ? []
          : [
              {
                scaled: strideScale.perErrorM > 0,
                marked: (quadrant & INSTANCE_SUB_GRID_STRIDES) !== 0,
                shares: words[k * per + 14] ?? 0,
              },
            ],
      );
    };
    const all = await masksOf("all");
    const ownGrids = await masksOf("own-grids");
    const off = await masksOf("off");
    expect([
      all.length > 0 &&
        all.every(({ scaled, marked, shares }) => scaled && !marked && shares === 0),
      ownGrids.length > 0 && off.length > 0,
      [...ownGrids, ...off].every(({ scaled, marked }) => !scaled && !marked),
    ]).toEqual([true, true, true]);
  });

  it("marks each of low's stand-ins, each on its own grid with no masks there, for the camera passes' cap where the stand-ins' strides are not off, with no scale, but one whose hold or fade sets its edges' k (R14.T3.c's stand-ins)", async () => {
    const lowStandInsOf = async (
      standInStrides: GeometrySettings["standInStrides"],
    ): Promise<
      Array<{
        readonly mask: boolean;
        readonly marked: boolean;
        readonly scaled: boolean;
        readonly held: boolean;
      }>
    > => {
      const made = await measuredPass(ERRORS, [], "low");
      made.pass.setGeometry({ ...SETTINGS.low.geometry, standInStrides });
      nearerUnbaked(made);
      const per = INSTANCE_RECORD_BYTES / 4;
      const words = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
      return stridesOf(made).flatMap(({ strideScale, quadrant }, k) => {
        const subWord = words[k * per + INSTANCE_SUB_PATCH_WORD] ?? 0;
        return subWord === 0
          ? []
          : [
              {
                mask: (subWord & TERRAIN_SUB_PATCH_MASK) !== 0,
                marked: (quadrant & INSTANCE_SUB_GRID_STRIDES) !== 0,
                scaled: strideScale.perErrorM > 0,
                held:
                  ((quadrant >>> 2) & 0xffff) !== 0 ||
                  (words[k * per + 9] ?? 0) !== 0 ||
                  (words[k * per + 10] ?? 0) !== 0,
              },
            ];
      });
    };
    const all = await lowStandInsOf("all");
    const ownGrids = await lowStandInsOf("own-grids");
    const off = await lowStandInsOf("off");
    expect([
      all.length > 0 && ownGrids.length > 0 && off.length > 0,
      [...all, ...ownGrids].some(({ held }) => !held),
      [...all, ...ownGrids, ...off].some(({ mask, scaled }) => mask || scaled),
      [...all, ...ownGrids].every(({ marked, held }) => marked === !held),
      off.some(({ marked }) => marked),
    ]).toEqual([true, true, false, true, false]);
  });

  /** Each measured interior record's slot, its holds at k = 0 and its onsets' shares word. */
  function heldOf(made: Made): Array<{ slot: number; held: number; shares: number }> {
    const per = INSTANCE_RECORD_BYTES / 4;
    const words = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
    return stridesOf(made).flatMap(({ strideScale, quadrant }, k) =>
      strideScale.perErrorM > 0
        ? [
            {
              slot: words[k * per + INSTANCE_SLOT_WORD] ?? -1,
              held: (quadrant >>> 2) & 0xf,
              shares: words[k * per + 14] ?? 0,
            },
          ]
        : [],
    );
  }

  it("fades a hold at k = 0 in as a finer patch arrives beside a measured patch's edge, its share 255 the frame it switches on", async () => {
    const made = await measuredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const before = new Map(heldOf(made).map(({ slot, held }) => [slot, held]));
    // Nearer, its patches baked: some refine beside patches that stay.
    const nearer = viewOver(20);
    for (let n = 0; n < 24; n += 1) {
      made.pass.step({ views: [passView(PRIMARY, nearer)], grounded: [] });
      made.lane().bakeDemand(made.pass);
    }
    made.pass.step({ views: [passView(PRIMARY, nearer)], grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(nearer, true));
    const onsets = heldOf(made).flatMap(({ slot, held, shares }) => {
      const was = before.get(slot);
      const fresh = was === undefined ? 0 : held & ~was;
      return fresh === 0 ? [] : [{ fresh, shares }];
    });
    expect([
      onsets.length > 0,
      onsets.every(({ fresh, shares }) =>
        [0, 1, 2, 3].every(
          (e) => ((fresh >>> e) & 1) === 0 || ((shares >>> (8 * e)) & 0xff) === 255,
        ),
      ),
    ]).toEqual([true, true]);
  });

  it("fades a hold at k = 0 out as the finer patch leaves, its byte 255 the frame it lets go", async () => {
    const made = await measuredPass();
    const nearer = viewOver(20);
    for (let n = 0; n < 24; n += 1) {
      made.pass.step({ views: [passView(PRIMARY, nearer)], grounded: [] });
      made.lane().bakeDemand(made.pass);
    }
    made.pass.step({ views: [passView(PRIMARY, nearer)], grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(nearer, true));
    const before = new Map(heldOf(made).map(({ slot, held }) => [slot, held]));
    // Back where it was, the finer patches coarsen away beside patches that stay.
    for (let n = 0; n < 24; n += 1) {
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      made.lane().bakeDemand(made.pass);
    }
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const releases = heldOf(made).flatMap(({ slot, held, shares }) => {
      const was = before.get(slot);
      const gone = was === undefined ? 0 : was & ~held;
      return gone === 0 ? [] : [{ gone, shares }];
    });
    expect([
      releases.length > 0,
      releases.every(({ gone, shares }) =>
        [0, 1, 2, 3].every(
          (e) => ((gone >>> e) & 1) === 0 || ((shares >>> (8 * e)) & 0xff) === 255,
        ),
      ),
    ]).toEqual([true, true]);
  });

  it("takes its scale at the bands' slewed tolerance, a jump of the view's τ moving it by at most a slew's step", async () => {
    const made = await measuredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const scaleOf = (): number =>
      stridesOf(made).find(({ strideScale }) => strideScale.perErrorM > 0)?.strideScale.perErrorM ??
      Number.NaN;
    const before = scaleOf();
    // Three times the view's τ: its target jumps, its bands' τ a step of the slew.
    const laxer = { ...passView(PRIMARY, NEAR), tauPx: 3 * TERRAIN_SETTINGS.high.tauPx };
    made.pass.step({ views: [laxer], grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const ratio = before / scaleOf();
    expect([ratio > 1, ratio <= 1 + BAND_SLEW_PER_STEP + 1e-6]).toEqual([true, true]);
  });

  it("draws at stride 1, every record without a scale or edges' bands, while the lever is off", async () => {
    const made = await measuredPass();
    made.pass.setGeometry(HIGH_AT_STRIDE_1);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const cull = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL) ?? fail();
    const records = stridesOf(made);
    expect(records.length).toBeGreaterThan(0);
    expect(records.every(noStrides)).toBe(true);
    expect(parseLitPass(cull.bindings.uniforms["passes"] ?? new Uint32Array(), 5).strideCap).toBe(
      1,
    );
  });

  it("gives each measured patch its level's scale, the same for each of a level's patches and one 1 ÷ τ′ for the view's", async () => {
    const made = await measuredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const levels = levelsOf(made);
    const byLevel = new Map<number, number>();
    const records = stridesOf(made)
      .map(({ strideScale }, n) => ({ strideScale, level: levels[n] ?? -1 }))
      .filter(({ strideScale }) => strideScale.perErrorM > 0);
    const unlike = records.filter(({ strideScale, level }) => {
      const held = byLevel.get(level) ?? strideScale.statedM;
      byLevel.set(level, held);
      return (
        strideScale.statedM !== held || strideScale.perErrorM !== records[0]?.strideScale.perErrorM
      );
    });
    expect([records.length > 0, unlike]).toEqual([true, []]);
  });

  it("holds each bake's blocks' e₂, e₄ and e₈ in its slot record", async () => {
    const made = await measuredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    // The last slot record written: its first two blocks' errors from byte 112.
    const slot = Uint32Array.from(wordsIn(made.engine, "terrain slot records")).buffer;
    expect([...new Float16Array(slot, 112, 6)]).toEqual([
      ...Float16Array.of(0.01, 0.02, 0.04, 0.01, 0.02, 0.04),
    ]);
  });

  it("gives each measured patch's edges their blocks' needs or larger", async () => {
    const made = await measuredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const records = stridesOf(made).filter(({ strideScale }) => strideScale.perErrorM > 0);
    const below = records.flatMap(({ strideScale, edges }) =>
      edges.filter((band, h) => {
        const need = strideNeedM(strideScale, [0.01, 0.02, 0.04][h % 3] ?? 0);
        return band !== NO_STRIDE_BAND && band < need;
      }),
    );
    expect([records.some(({ edges }) => edges.some((band) => band > 0)), below]).toEqual([
      true,
      [],
    ]);
  });

  it("gives an edge a finer patch touches none at level 2 where the finer patch is beside it", async () => {
    const made = await measuredPass();
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    // An edge a finer patch touches is held at k = 0 at level 2 along its length, the finer patch
    // beside some of its blocks, the others beside no drawn patch.
    const finerTouched = stridesOf(made)
      .filter(({ strideScale }) => strideScale.perErrorM > 0)
      .flatMap(({ edges, quadrant }) =>
        [0, 1, 2, 3]
          .filter((e) => ((quadrant >>> (2 + e)) & 1) === 1)
          .map((e) =>
            Array.from({ length: 8 }, (_, t) => edges[edgeBandIndex(e, t, 2)]).some(
              (band) => band === NO_STRIDE_BAND,
            ),
          ),
      );
    expect([finerTouched.length > 0, finerTouched.every(Boolean)]).toEqual([true, true]);
  });

  it("culls the lit pass up to stride 8 and draws every stride's buckets", async () => {
    const made = await measuredPass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const cull = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL) ?? fail();
    const lit = 5 * TEMPLATES;
    expect([
      parseLitPass(cull.bindings.uniforms["passes"] ?? new Uint32Array(), 5).strideCap,
      named(frame.draws).filter(
        (draw) => Array.isArray(draw) && typeof draw[2] === "number" && draw[2] >= lit,
      ),
    ]).toEqual([
      8,
      // By its still form, the frame mixing no heights (R10.T16.i).
      Array.from({ length: 8 }, (_, t) => [
        "terrain clusters",
        `terrain-lit-cluster-${made.pass.layout.vertexPath}-still`,
        lit + t,
        `terrain interior instances (${PRIMARY})`,
      ]),
    ]);
  });

  it("gives the overlay its depth by its own pass at stride 1, even where its lever is redraw", async () => {
    const made = await measuredPass();
    made.pass.setGeometry({ ...STRIDED, overlayDepth: "redraw" });
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const overlay = 4 * TEMPLATES;
    expect(
      (frame.overlayDepth?.clusters ?? []).map((draw) => (draw.indirect?.offsetBytes ?? -20) / 20),
    ).toEqual([overlay, overlay + 4]);
  });

  it("gives a bake without measured errors no scale and no edges' bands", async () => {
    const made = await measuredPass(null);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const records = stridesOf(made);
    expect(records.length).toBeGreaterThan(0);
    expect(records.every(noStrides)).toBe(true);
  });

  it("holds every patch that may lie in a contact's forced region at stride 1, and no other", async () => {
    const [x, y, z] = surfacePoint(PLANET.figure, OVER, 0);
    const contact: GroundContact = { positionM: { x, y, z }, radiusM: 20 };
    const made = await measuredPass(ERRORS, [contact]);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const finest = stridesOf(made).map(({ quadrant }) => (quadrant & INSTANCE_FINEST) !== 0);
    expect([finest.some(Boolean), finest.every(Boolean)]).toEqual([true, false]);
    // Its scale stands: its vertices still morph, held at the contact as the CDLOD morph is.
    expect(stridesOf(made).every(({ strideScale }) => strideScale.perErrorM > 0)).toBe(true);
  });

  describe("the triangle budget and the count of strides past τ′ (plan R14, R14.T3.e)", () => {
    const BUDGETED: GeometrySettings = { ...STRIDED, terrainBudget: "triangles" };
    /** The lit cull's `CullParams` word that names the arguments' first count of strides past τ′. */
    const overWordOf = (made: Made): number => {
      const params = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL)?.bindings
        .uniforms["params"];
      return params === undefined ? Number.NaN : (new Uint32Array(params.buffer)[55] ?? Number.NaN);
    };
    const litToleranceOf = (made: Made): number =>
      parseLitPass(
        made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL)?.bindings.uniforms[
          "passes"
        ] ?? new Uint32Array(),
        5,
      ).toleranceRad;

    it("counts its strides past τ′ in the lit pass's cull wherever the strides are on, whatever the budget, at the view's τ′ over its px per radian, copying the counts back every frame, and nothing with the strides off (research-leans-42)", async () => {
      const made = await measuredPass();
      made.pass.setGeometry(PATCHED);
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      const counted = (): [boolean, boolean, number] => {
        const copies = overCopies(made);
        made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
        const { viewport, fovXRad } = frameOf(NEAR, true).view;
        // Design note 3's τ′, unslewed: the view's τ, raised where the budget binds.
        const tauPrimePx = effectiveTauPx(
          TERRAIN_SETTINGS.high.tauPx,
          made.pass.lastStep?.selection.limitExcess ?? Number.NaN,
          1,
        );
        const tauRad = (2 * Math.tan(fovXRad / 2) * tauPrimePx) / viewport.widthPx;
        return [
          overWordOf(made) > 0,
          Math.abs(litToleranceOf(made) / tauRad - 1) < 1e-6,
          overCopies(made) - copies,
        ];
      };
      const patches = counted();
      made.pass.setGeometry(BUDGETED);
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      const triangles = counted();
      made.pass.setGeometry({ ...SETTINGS.high.geometry, strides: "off" });
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
      expect([patches, triangles, [overWordOf(made), litToleranceOf(made)]]).toEqual([
        [true, true, 1],
        [true, true, 1],
        [0, 0],
      ]);
    });

    /**
     * The twin's count of strides the lit pass drew past its τ′ in `made`'s last frame, from the
     * frame's dispatch, instance records, cull inputs and slots' cluster records.
     */
    function twinPastOf(made: Made): number {
      const cull = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL) ?? fail();
      const lit = parseLitPass(cull.bindings.uniforms["passes"] ?? new Uint32Array(), 5);
      const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
      const words = wordsIn(made.engine, `terrain interior instances (${PRIMARY})`);
      const per = INSTANCE_RECORD_BYTES / 4;
      const strides = stridesOf(made);
      const records = clusterRecordsOf(made.engine);
      // The records' measured errors, e₁ to e₈ in half floats, beside their places.
      const errorsBySlot = new Map<number, DataView>();
      for (const write of made.engine.writes) {
        if (write.buffer === "terrain cluster records") {
          errorsBySlot.set(
            write.offsetBytes / (TERRAIN_PAGE_CLUSTERS * 32),
            new DataView(write.data.slice().buffer),
          );
        }
      }
      // The dispatch's instances alone, a workgroup each.
      const count = Array.isArray(cull.workgroups) ? (cull.workgroups[0] ?? 0) : 0;
      const counts = cullOverTwin({
        instances: instanceOrigins(made.engine, `terrain interior instances (${PRIMARY})`)
          .slice(0, count)
          .map(({ originM, page }, n) => ({
            originM,
            page,
            passMask: (inputs[2 * n] ?? 0) & ~CULL_INPUT_WHOLE,
            unitM: f32Of(inputs[2 * n + 1] ?? 0),
            whole: ((inputs[2 * n] ?? 0) & CULL_INPUT_WHOLE) !== 0,
            subWord: words[per * n + 8] ?? 0,
            quadrantWord: strides[n]?.quadrant ?? 0,
            holdFadesWord: words[per * n + 14] ?? 0,
            strideScale: strides[n]?.strideScale ?? NO_STRIDE_SCALE,
            edgeBandsM: Float64Array.from(strides[n]?.edges ?? []),
          })),
        records: (page, cluster) => {
          const record = records(page, cluster);
          const view = errorsBySlot.get(page);
          const at = cluster * 32;
          const flags = view?.getUint32(at + 28, true) ?? 0;
          if (view === undefined || (flags & CLUSTER_FLAG_MEASURED) === 0) {
            return record;
          }
          const x = view.getUint32(at + 16, true);
          const y = view.getUint32(at + 20, true);
          return { ...record, strideErrorBits: [x & 0xffff, x >>> 16, y & 0xffff, y >>> 16] };
        },
        clustersPerPage: TERRAIN_CLUSTERS_PER_PAGE,
        rotationColumns: [
          { x: 1, y: 0, z: 0 },
          { x: 0, y: 1, z: 0 },
          { x: 0, y: 0, z: 1 },
        ],
        passes: [null, null, null, null, null, lit],
        places: Array.from({ length: 6 }, (_, k) => ({ bucketBase: TEMPLATES * k, listBase: 0 })),
        templates: TEMPLATES,
        viewAxis: { x: 0, y: 0, z: -1 },
      });
      return counts.get(5) ?? 0;
    }

    it("under the patches' budget with the strides on, states DETAIL LIMITED while a tightened τ′ leaves the strides, a slew behind it, past it, and clears once they catch up (research-leans-42)", async () => {
      // Rough ground, e₂ to e₈ of 2 to 8 m, so that a stride's own error is not lost in E_L's.
      const rough = new Float16Array(4 * TERRAIN_CLUSTERS_PER_PAGE);
      for (let c = 0; c < TERRAIN_CLUSTERS_PER_PAGE; c += 1) {
        rough.set([0, 2, 4, 8], 4 * c);
      }
      const made = await measuredPass(rough);
      made.pass.setGeometry(PATCHED);
      // At lax tolerances, so that the selection's budget binds nowhere and states nothing.
      const at = (tauPx: number): TerrainPassView => ({ ...passView(PRIMARY, NEAR), tauPx });
      // The bands slew 2% a step: from the pass's 1 px to 16 px in some 140 steps.
      const settleBands = (tauPx: number): void => {
        for (let n = 0; n < 400 && (n === 0 || !made.pass.bandsAtTarget(PRIMARY)); n += 1) {
          made.pass.step({ views: [at(tauPx)], grounded: [] });
          made.lane().bakeDemand(made.pass);
          made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
        }
      };
      settleBands(16);
      // The device's counts as the twin made them of the latest frame drawn, as each read comes back.
      let answer = 0;
      vi.spyOn(made.engine, "readBuffer").mockImplementation(() =>
        Promise.resolve().then(() => {
          const words = new Uint32Array(8);
          words[5] = answer;
          return words.buffer;
        }),
      );
      // A step and its frame, the twin's count of it read back by the next frame's copy, and the
      // conditions of the frame after, which reads it.
      const frame = async (tauPx: number): Promise<[number, boolean]> => {
        made.pass.step({ views: [at(tauPx)], grounded: [] });
        made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
        answer = twinPastOf(made);
        made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
        await Promise.resolve();
        await Promise.resolve();
        const conditions = made.pass.drawFor(PRIMARY, frameOf(NEAR, true)).conditions;
        return [answer, conditions.detailLimited];
      };
      const steady = await frame(16);
      // τ′ halves at once; the strides' scale follows it 2% a step.
      const [past, limited] = await frame(8);
      settleBands(8);
      const caught = await frame(8);
      expect([steady, past > 0, limited, caught, made.pass.lastStep?.selection.limited]).toEqual([
        [0, false],
        true,
        true,
        [0, false],
        false,
      ]);
    });

    it("states DETAIL LIMITED once a count of strides past τ′ comes back above 0, and not while it is 0", async () => {
      const made = await measuredPass();
      made.pass.setGeometry(BUDGETED);
      // At a lax τ, so that the budget does not bind and the selection alone states nothing.
      const lax = { ...passView(PRIMARY, NEAR), tauPx: 64 };
      for (let n = 0; n < 4; n += 1) {
        made.pass.step({ views: [lax], grounded: [] });
      }
      // The lit pass's counts, its own and its late pass's, as the copy reads them back.
      const answer = new Uint32Array(8);
      vi.spyOn(made.engine, "readBuffer").mockImplementation(() =>
        Promise.resolve(answer.slice().buffer),
      );
      const limited = async (): Promise<boolean> => {
        const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
        await Promise.resolve();
        await Promise.resolve();
        return frame.conditions.detailLimited;
      };
      const before = [await limited(), await limited()];
      answer[5] = 3;
      await limited();
      const after = await limited();
      answer[5] = 0;
      await limited();
      expect([made.pass.lastStep?.selection.limited, before, after, await limited()]).toEqual([
        false,
        [false, false],
        true,
        false,
      ]);
    });

    it("makes its counts' kernel and ring again after a restore, and counts as a fresh pass does (Design note 16)", async () => {
      const made = await measuredPass();
      made.pass.setGeometry(BUDGETED);
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
      const kernels = (): number =>
        made.engine.kernelPairs.filter((pair) => pair.name === "cluster over counts").length;
      const before = kernels();
      const created: string[] = [];
      const off = made.engine.onAllocation((event) => {
        if (event.kind === "created") {
          created.push(event.name);
        }
      });
      made.engine.loseDevice();
      made.engine.restore();
      await made.pass.ready();
      made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
      made.lane().clusterErrors = ERRORS;
      stream(made, [passView(PRIMARY, NEAR)], 24);
      made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
      off();
      const fresh = await measuredPass();
      fresh.pass.setGeometry(BUDGETED);
      fresh.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      fresh.pass.drawFor(PRIMARY, frameOf(NEAR, true));
      expect([
        kernels() > before,
        created.includes(`terrain clusters (${PRIMARY}) over counts 1`),
        overWordOf(made),
        litToleranceOf(made),
      ]).toEqual([true, true, overWordOf(fresh), litToleranceOf(fresh)]);
    });

    it("counts nothing on the low setting, which states DETAIL LIMITED wherever it draws lit terrain", async () => {
      const made = await litPass("low");
      made.pass.setReducedMotion(true);
      made.pass.setGeometry(SETTINGS.low.geometry);
      made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
      made.lane().clusterErrors = ERRORS;
      for (let n = 0; n < 24; n += 1) {
        made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
        made.lane().bakeDemand(made.pass);
      }
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
      expect([
        SETTINGS.low.geometry.terrainBudget,
        made.engine.dispatched.some((d) => d.kernel === LIT_CULL),
        overWordOf(made),
        overCopies(made),
        frame.conditions.detailLimited,
      ]).toEqual(["triangles", true, 0, 0, true]);
    });

    it("selects as the patches' budget alone at the setting's held cache, one view's patches each within a whole grid", async () => {
      const made = await measuredPass();
      made.pass.setGeometry(PATCHED);
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      const selection = (): [string[], boolean | undefined] => [
        [...(made.pass.lastStep?.selection.patches.keys() ?? [])],
        made.pass.lastStep?.selection.limited,
      ];
      const patches = selection();
      made.pass.setGeometry(BUDGETED);
      made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
      expect(selection()).toEqual(patches);
    });
  });
});

/** A view's pyramid as its renderer holds one: its builds counted by hand, its frame a buffer. */
interface HandPyramid {
  builds: number;
  frame: PyramidFrame | null;
}

/** A pyramid of a 64 × 48 depth on `made`'s engine, built `builds` times (plan R14, R14.T3.d). */
function pyramidOf(made: Made, builds = 0): HandPyramid {
  const size = { widthPx: 64, heightPx: 48 };
  const levels = pyramidLevels(size);
  const buffer = made.engine.createBuffer({
    name: "view depth pyramid",
    bytes: 4 * pyramidTexels(levels),
    usage: BUFFER_USAGE.STORAGE,
    category: "geometry-clusters",
  });
  return {
    builds,
    frame: { buffer, levels, size, projection: pyramidProjectionOf(Math.PI / 3, size) },
  };
}

describe("the two-pass occlusion on the cluster path (plan R14, R14.T3.d)", () => {
  const OCCLUDED: GeometrySettings = { ...HIGH_AT_STRIDE_1, occlusion: "hzb" };

  /** The kind of the last lit cull's pass `index`. */
  function lastKind(made: Made, index: number): number {
    const cull = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL) ?? fail();
    return cull.bindings.uniforms["passes"]?.[(index * CULL_PASS_BYTES) / 4] ?? -1;
  }

  /** How many times the view's whole visibility was cleared. */
  function drops(made: Made): number {
    const bytes = made.pass.layout.slots.slotCount * CLUSTER_HISTORY_SLOT_BYTES;
    return made.engine.writes.filter(
      (w) => w.buffer === `terrain clusters (${PRIMARY}) visibility` && w.bytes === bytes,
    ).length;
  }

  it("culls the lit pass as the occlusion's early pass where the frame takes it, and as before where it does not or the lever is off", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(OCCLUDED);
    const occlusion = { pyramid: pyramidOf(made), fresh: true };
    made.pass.drawFor(PRIMARY, { ...frameOf(NEAR, true), occlusion });
    const early = lastKind(made, CLUSTER_LIT_PASS);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const without = [lastKind(made, CLUSTER_LIT_PASS), made.pass.lateDrawFor(PRIMARY).length];
    made.pass.setGeometry(SETTINGS.high.geometry);
    made.pass.drawFor(PRIMARY, { ...frameOf(NEAR, true), occlusion });
    expect([
      early,
      without,
      lastKind(made, CLUSTER_LIT_PASS),
      made.pass.lateDrawFor(PRIMARY),
    ]).toEqual([4, [3, 0], 3, []]);
  });

  it("draws the rest after the first raster: the late pass against the view's pyramid, compacted into its own buckets, in the lit pass's order", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(OCCLUDED);
    const pyramid = pyramidOf(made);
    const frame = made.pass.drawFor(PRIMARY, {
      ...frameOf(NEAR, true),
      occlusion: { pyramid, fresh: true },
    });
    pyramid.builds += 1;
    const from = made.engine.dispatched.length;
    const late = made.pass.lateDrawFor(PRIMARY);
    const dispatched = made.engine.dispatched.slice(from);
    const cull = dispatched[0] ?? fail();
    const passes = cull.bindings.uniforms["passes"] ?? new Uint32Array();
    const hzb = cull.bindings.uniforms["hzb"] ?? new Uint32Array();
    const buckets = TERRAIN_CLUSTER_MESH.templates.length;
    const lit = frame.draws.filter((draw) => draw.mesh.name === "terrain clusters");
    // The late pass's index in its dispatch, whose bit every record entering the lit pass holds.
    const kinds = Array.from({ length: 8 }, (_, p) => passes[(p * CULL_PASS_BYTES) / 4] ?? 0);
    const at = kinds.indexOf(5);
    const inputs = wordsIn(made.engine, `terrain clusters (${PRIMARY}) cull inputs`);
    const entering = Array.from({ length: inputs.length / 2 }, (_, n) => inputs[2 * n] ?? 0).filter(
      (word) => (word & (1 << CLUSTER_LIT_PASS)) !== 0,
    );
    expect([
      dispatched.map((d) => d.kernel),
      [at, entering.length > 0 && entering.every((word) => (word & (1 << at)) !== 0)],
      cull.bindings.uniforms["params"]?.[15],
      [cull.bindings.buffers["pyramid"]?.name, hzb[14]],
      dispatched.slice(1).map((d) => d.bindings.uniforms["params"]?.[4]),
      late.map((draw) => draw.indirect?.offsetBytes),
      late.map((draw) => draw.material.name),
      made.pass.lateDrawFor(PRIMARY),
    ]).toEqual([
      [LIT_CULL, "cluster compaction", "cluster compaction", "cluster compaction"],
      [CLUSTER_LIT_PASS, true],
      CLUSTER_LIT_PASS + 1,
      ["view depth pyramid", pyramid.frame?.levels.length],
      [0, 1, 2].map(() => CLUSTER_LIT_LATE_PASS * buckets),
      lit.map((draw) => (draw.indirect?.offsetBytes ?? 0) + buckets * BUCKET_ARGS_BYTES),
      lit.map((draw) => draw.material.name),
      [],
    ]);
  });

  it("refuses nothing where the renderer built no pyramid", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(OCCLUDED);
    made.pass.drawFor(PRIMARY, {
      ...frameOf(NEAR, true),
      occlusion: { pyramid: { builds: 0, frame: null }, fresh: true },
    });
    made.pass.lateDrawFor(PRIMARY);
    const cull = made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL) ?? fail();
    expect([
      cull.bindings.buffers["pyramid"]?.name,
      cull.bindings.uniforms["hzb"]?.every((word) => word === 0),
    ]).toEqual([`terrain clusters (${PRIMARY}) no pyramid`, true]);
  });

  it("drops the visibility where the frame says, at another pyramid and after a frame the view did not draw, and keeps it otherwise", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(OCCLUDED);
    const pyramid = pyramidOf(made);
    const frame = (occlusion: {
      readonly pyramid: HandPyramid;
      readonly fresh: boolean;
    }): number => {
      const before = drops(made);
      made.pass.drawFor(PRIMARY, { ...frameOf(NEAR, true), occlusion });
      occlusion.pyramid.builds += 1;
      made.pass.lateDrawFor(PRIMARY);
      return drops(made) - before;
    };
    const first = frame({ pyramid, fresh: true });
    const next = frame({ pyramid, fresh: false });
    const cut = frame({ pyramid, fresh: true });
    // A frame of the view's this pass did not draw: a hand-over, a view's switch.
    pyramid.builds += 1;
    const gap = frame({ pyramid, fresh: false });
    const another = frame({ pyramid: pyramidOf(made, pyramid.builds), fresh: false });
    expect([first, next, cut, gap, another]).toEqual([1, 0, 1, 1, 1]);
  });

  it("redraws the late pass's clusters for the overlay's depth beside the lit pass's", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry({ ...OCCLUDED, overlayDepth: "redraw" });
    const frame = made.pass.drawFor(PRIMARY, {
      ...frameOf(NEAR, true),
      occlusion: { pyramid: pyramidOf(made), fresh: true },
    });
    const buckets = TERRAIN_CLUSTER_MESH.templates.length;
    const late = CLUSTER_LIT_LATE_PASS * buckets * BUCKET_ARGS_BYTES;
    const offsets = (frame.overlayDepth?.clusters ?? []).map((d) => d.indirect?.offsetBytes ?? 0);
    expect([offsets.some((o) => o < late), offsets.some((o) => o >= late)]).toEqual([true, true]);
  });

  it("draws no late pass culled on a device that was lost since (Design note 16)", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(OCCLUDED);
    made.pass.drawFor(PRIMARY, {
      ...frameOf(NEAR, true),
      occlusion: { pyramid: pyramidOf(made), fresh: true },
    });
    made.engine.loseDevice();
    made.engine.restore();
    expect(made.pass.lateDrawFor(PRIMARY)).toEqual([]);
  });
});

/** Lets the pass's compiles' promise chains settle: the counting engine answers each at once. */
async function settle(): Promise<void> {
  for (let hop = 0; hop < 10; hop += 1) {
    // Each hop waits on the last.
    // oxlint-disable-next-line no-await-in-loop
    await Promise.resolve();
  }
}

/** A draw's mesh, material, bucket and list base. */
function bucketOf(draw: DrawItem): unknown[] {
  return [
    draw.mesh.name,
    draw.material.name,
    draw.indirect === undefined ? null : draw.indirect.offsetBytes / BUCKET_ARGS_BYTES,
    [...(draw.uniforms["clusterBase"] ?? [])],
  ];
}

describe("the bodies pass's opaque draws first (plan R14, R14.T2.e)", () => {
  it("leads the lit pass's draws with its opaque buckets on the cluster path, at stride 1 and with strides", async () => {
    const made = await clusteredPass("low");
    const counts = (geometry: GeometrySettings): unknown => {
      made.pass.setGeometry(geometry);
      const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
      const lead = frame.draws.slice(0, frame.opaqueDraws).map((draw) => draw.material.name);
      return [
        frame.opaqueDraws,
        lead.every((name) => name.startsWith("terrain-lit-cluster-")),
        frame.draws.slice(frame.opaqueDraws).every((draw) => !lead.includes(draw.material.name)),
      ];
    };
    expect([
      counts({ ...SETTINGS.low.geometry, strides: "off" }),
      counts({ ...SETTINGS.low.geometry, strides: "measured" }),
    ]).toEqual([
      [2, true, true],
      [8, true, true],
    ]);
  });

  it("leads them with the interior patches' draw where they draw by the interior mesh, and with none by the way back", async () => {
    const made = await clusteredPass("low");
    made.pass.setGeometry({ ...SETTINGS.low.geometry, litClusters: false });
    const split = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    made.pass.setGeometry(GEOMETRY_BEFORE_R14);
    const back = made.pass.drawFor(PRIMARY, frameOf(NEAR, false));
    expect([
      split.opaqueDraws,
      split.draws[0]?.mesh.name,
      back.opaqueDraws,
      back.draws.length > 0,
    ]).toEqual([1, "terrain lit interior patch", 0, true]);
  });
});

describe("the lit pass by the visibility buffer's ID raster (plan R14, R14.T2.c)", () => {
  const VISIBILITY: GeometrySettings = { ...SETTINGS.high.geometry, shading: "visibility" };

  /** `made` shading by the visibility buffer: its ID form asked at a frame, then made. */
  async function byIds(made: Made): Promise<Made> {
    made.pass.setGeometry(VISIBILITY);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    await settle();
    return made;
  }

  it("draws forward, with no frame by IDs, where the geometry shades forward, and until its ID form is made", async () => {
    const made = await clusteredPass();
    const forward = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    made.pass.setGeometry(VISIBILITY);
    const asked = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([forward.visibility, asked.visibility, asked.draws.length > 0]).toEqual([
      null,
      null,
      true,
    ]);
    // Asked once, at the first frame that shades by IDs, and never on the forward setting.
    expect(
      made.engine.compiles
        .filter((c) => c.name.startsWith("terrain-id-cluster-"))
        .map((c) => c.name),
    ).toEqual([`terrain-id-cluster-${made.pass.layout.vertexPath}`]);
  });

  it("rasterises the lit pass's opaque buckets, the lit cluster form's, by the de-indexed ID form into r32uint, leaving the forward draws as they were", async () => {
    const made = await byIds(await clusteredPass());
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const raster = frame.visibility?.raster ?? [];
    expect(raster.map(bucketOf)).toEqual(
      frame.draws.map((draw) => [
        "terrain cluster ids",
        `terrain-id-cluster-${path}-still`,
        ...bucketOf(draw).slice(2),
      ]),
    );
    expect([
      frame.visibility?.forward,
      frame.draws.map((draw) => draw.material.name),
      raster.map((draw) => draw.storageBuffers?.["clusterLists"]?.name),
      made.engine.compiles.find((c) => c.name === `terrain-id-cluster-${path}`)?.targets,
    ]).toEqual([
      [],
      frame.draws.map(() => `terrain-lit-cluster-${path}-still`),
      raster.map(() => `terrain clusters (${PRIMARY}) lists`),
      ["r32uint"],
    ]);
  });

  /** A clustered pass on a device with `primitive-index`, its ID forms refused where `refused`. */
  async function primitivePass(refused: (name: string) => boolean = () => false): Promise<Made> {
    const limits = {
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    };
    const device = await new FakeAdapter({
      info: SWIFTSHADER_INFO,
      features: ["primitive-index"],
      ...limits,
    }).requestDevice({ requiredFeatures: ["primitive-index"], requiredLimits: { ...limits } });
    const engine = new CountingRenderEngine(device);
    engine.failMaterials(refused);
    const made = await litPass("high", engine);
    made.pass.setReducedMotion(true);
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    stream(made, [passView(PRIMARY, NEAR)], 24);
    return byIds(made);
  }

  it("draws de-indexed where the primitive_index form is refused, stating no refusal of the path (Design note 6)", async () => {
    const made = await primitivePass((name) => name.startsWith("terrain-id-cluster-primitive"));
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    expect([
      (frame.visibility?.raster ?? []).map((draw) => [draw.mesh.name, draw.material.name]),
      made.pass.clusterRefusal,
    ]).toEqual([
      frame.draws.map(() => ["terrain cluster ids", `terrain-id-cluster-${path}-still`]),
      null,
    ]);
  });

  it("resolves the raster's pixels by one full-screen draw over the lit draw's bindings, the lists' layout and the view's resolve frame (R14.T2.d)", async () => {
    const made = await byIds(await clusteredPass());
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    const [resolve, ...others] = frame.visibility?.resolve ?? [];
    const lit = frame.draws[0];
    const entries = PASS_ENTRIES(made);
    expect([
      others,
      resolve?.mesh.name,
      resolve?.material.name,
      resolve?.indirect,
      [...(resolve?.uniforms["resolveLists"] ?? [])],
      resolve?.uniforms["cascadeViewAxis"] === lit?.uniforms["cascadeViewAxis"],
      resolve?.textures["resolveFrame"]?.name,
      resolve?.storageBuffers?.["clusterLists"]?.name,
    ]).toEqual([
      [],
      "resolve screen",
      `terrain-resolve-${path}-still`,
      undefined,
      // SwiftShader's sub-pixel grid, 16 cells a pixel.
      [entries, 0, made.pass.layout.slots.slotCount > 0 ? 7 * entries : 0, 16],
      true,
      `terrain resolve frame (${PRIMARY})`,
      `terrain clusters (${PRIMARY}) lists`,
    ]);
  });

  it("takes the primitive_index form over the terrain cluster mesh on a device with primitive-index", async () => {
    const made = await primitivePass();
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const path = made.pass.layout.vertexPath;
    expect([
      made.engine.capabilities.primitiveIndex,
      (frame.visibility?.raster ?? []).map((draw) => [draw.mesh.name, draw.material.name]),
    ]).toEqual([
      true,
      frame.draws.map(() => ["terrain clusters", `terrain-id-cluster-primitive-${path}-still`]),
    ]);
  });

  it("with the two-pass occlusion rasterises the early pass's opaque buckets, then at its late draws by IDs the late pass's, the one resolve over both (R14.T2.d)", async () => {
    const made = await byIds(await clusteredPass());
    made.pass.setGeometry({ ...VISIBILITY, occlusion: "hzb" });
    const occlusion = { pyramid: pyramidOf(made), fresh: true };
    const frame = made.pass.drawFor(PRIMARY, { ...frameOf(NEAR, true), occlusion });
    const late = made.pass.lateDrawFor(PRIMARY, true);
    const raster = frame.visibility?.raster ?? [];
    const buckets = TERRAIN_CLUSTER_MESH.templates.length;
    const entries = PASS_ENTRIES(made);
    // A frame drawn forward asks the late pass's lit draws, as without the visibility buffer.
    made.pass.drawFor(PRIMARY, { ...frameOf(NEAR, true), occlusion });
    const litLate = made.pass.lateDrawFor(PRIMARY);
    const path = made.pass.layout.vertexPath;
    expect([
      raster.length > 0,
      late.map((draw) => [draw.mesh.name, draw.material.name]),
      late.map((draw) => draw.indirect?.offsetBytes),
      late.map((draw) => draw.uniforms["clusterBase"]?.[0]),
      frame.visibility?.resolve[0]?.uniforms["resolveLists"]?.[2],
      litLate.map((draw) => draw.material.name),
    ]).toEqual([
      true,
      raster.map((draw) => [draw.mesh.name, draw.material.name]),
      raster.map((draw) => (draw.indirect?.offsetBytes ?? 0) + buckets * BUCKET_ARGS_BYTES),
      raster.map((draw) => (draw.uniforms["clusterBase"]?.[0] ?? 0) + entries),
      7 * entries,
      litLate.map(() => `terrain-lit-cluster-${path}-still`),
    ]);
    expect(litLate.length).toBe(late.length);
  });

  it("asks its ID form again after a restore, its first frame by IDs a fresh pass's (Design note 16)", async () => {
    const shapeOf = (frame: TerrainViewFrame): unknown => ({
      raster: frame.visibility?.raster.map(drawShape) ?? null,
      resolve:
        frame.visibility?.resolve.map((draw) => [
          drawShape(draw),
          Object.entries(draw.textures).map(([name, texture]) => [name, texture.name]),
        ]) ?? null,
      forward: frame.visibility?.forward.map(drawShape) ?? null,
    });
    const made = await byIds(await clusteredPass());
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    made.lane().layers = { ...layers(), surveyMask: new Uint8Array(65 * 65).fill(255) };
    stream(made, [passView(PRIMARY, NEAR)], 24);
    // The restored device's first frame asks the form again, and draws forward until it is made.
    const asking = made.pass.drawFor(PRIMARY, frameOf(NEAR, true)).visibility;
    await settle();
    const restored = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const fresh = await byIds(await clusteredPass());
    const freshFrame = fresh.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([asking, restored.visibility !== null]).toEqual([null, true]);
    expect(shapeOf(restored)).toEqual(shapeOf(freshFrame));
  });

  it("draws forward where its resolve is refused, and states the refusal (R14.T2.d)", async () => {
    const made = await clusteredPass();
    made.engine.failMaterials((name) => name.startsWith("terrain-resolve"));
    await byIds(made);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([frame.visibility, made.pass.clusterRefusal?.startsWith("the resolve form")]).toEqual([
      null,
      true,
    ]);
  });

  it("resolves a frame whose lighting carries its ground's emission by the resolve's emissive form, once made (R14.T2.d)", async () => {
    const made = await byIds(await clusteredPass());
    const emitting: TerrainViewFrameInput = {
      ...frameOf(NEAR, true),
      lighting: {
        ...STARLIT,
        emission: {
          field: surfaceTemperatureFieldOf(lavaWorldSurface()),
          star: { x: 0, y: 0, z: 1 },
          meltClasses: [],
        },
      },
    };
    made.pass.drawFor(PRIMARY, emitting);
    await settle();
    const frame = made.pass.drawFor(PRIMARY, emitting);
    expect(frame.visibility?.resolve.map((draw) => draw.material.name)).toEqual([
      `terrain-resolve-emissive-${made.pass.layout.vertexPath}-still`,
    ]);
  });

  it("draws forward where its ID form is refused, and states the refusal", async () => {
    const made = await clusteredPass();
    made.engine.failMaterials((name) => name.startsWith("terrain-id-cluster"));
    await byIds(made);
    const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    expect([frame.visibility, made.pass.clusterRefusal?.startsWith("the id-cluster form")]).toEqual(
      [null, true],
    );
  });
});

describe("culling a caster by its cascade's light-space box (R10.T9.b; D4)", () => {
  /** A hand-made set: 2 m over a Moon-sized body, the sun 20° up, its reach for 10 km of relief. */
  const SET: CascadeSet = fitCascades({
    cameraBodyFixedM: { x: 0, y: 0, z: 1_737_402 },
    orientationBodyFixed: lookAlong({ x: 1, y: 0, z: -0.2 }, { x: 0, y: 0, z: 1 }),
    rotation: IDENTITY_ROTATION,
    fovXRad: Math.PI / 3,
    aspect: 16 / 9,
    heightAboveGroundM: 2,
    sunDirectionBodyFixed: { x: Math.cos(20 * RAD), y: 0, z: Math.sin(20 * RAD) },
    datumNormalBodyFixed: { x: 0, y: 0, z: 1 },
    bodyRadiusM: 1_737_400,
    reliefRangeM: 10_000,
    horizonTexelM: 1,
    depthClipControl: false,
  });

  /** The place `v` from cascade `index`'s snapped centre, in the light's axes, less the camera. */
  function at(index: CascadeIndex, v: Vec3): Vec3 {
    const o = SET.cascades[index].centreFromCameraLightM;
    const { x, y, z } = SET.basis;
    return add(add(scale(x, o.x + v.x), scale(y, o.y + v.y)), scale(z, o.z + v.z));
  }

  it.each([0, 1, 2, 3] as const)(
    "keeps a patch inside cascade %i's box, or within the reach towards the sun, and drops one beside, behind or beyond it",
    (index) => {
      const r = SET.cascades[index].radiusM;
      const reach = SET.casterReachM;
      const rho = 5;
      const cases: ReadonlyArray<readonly [Vec3, boolean]> = [
        [{ x: 0, y: 0, z: 0 }, true],
        [{ x: r + rho - 0.5, y: 0, z: 0 }, true],
        [{ x: r + rho + 0.5, y: 0, z: 0 }, false],
        [{ x: 0, y: -(r + rho + 0.5), z: 0 }, false],
        [{ x: 0, y: 0, z: -(r + rho - 0.5) }, true],
        [{ x: 0, y: 0, z: -(r + rho + 0.5) }, false],
        [{ x: 0, y: 0, z: r + reach + rho - 0.5 }, true],
        [{ x: 0, y: 0, z: r + reach + rho + 0.5 }, false],
      ];
      expect(cases.map(([v]) => castsInto(SET, index, at(index, v), rho))).toEqual(
        cases.map(([, kept]) => kept),
      );
    },
  );

  it("takes a reach of thousands of metres towards a low sun, so that a far caster still casts", () => {
    expect(SET.casterReachM).toBeGreaterThan(10_000);
  });
});

/**
 * The camera's height in frame `n` of the centres' cases, m: above 100 m the shadowed range follows
 * h, so the texel changes as the camera climbs.
 */
function heightOf(n: number): number {
  return 150 + n;
}

describe("the view's snapped centres (R10.T9.b; D6(d); ruling 7 amended)", () => {
  it("carries them from frame to frame, and drops them after a frame without the cascades", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    const fit = (n: number, takes = true): TerrainViewFrame =>
      made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(n)), takes, n));
    const lattice = (frame: TerrainViewFrame, n: number): boolean =>
      onLattice(cascadesOf(frame).set, viewOver(heightOf(n)).cameraM);
    const first = fit(0);
    const carried = fit(1);
    fit(2, false);
    const dropped = fit(3);
    expect([lattice(first, 0), lattice(carried, 1), lattice(dropped, 3)]).toEqual([
      true,
      false,
      true,
    ]);
  });

  it("keeps them through a frame at w = 0, which still fits", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(0)), true));
    const orbit = made.pass.drawFor(PRIMARY, frameOf(ORBIT, true));
    const back = made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(1)), true));
    expect([
      cascadesOf(orbit).set.weight,
      onLattice(cascadesOf(back).set, viewOver(heightOf(1)).cameraM),
    ]).toEqual([0, false]);
  });

  it("drops them for a view new to the pass, and when the pass is remade after a loss", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    const primary = passView(PRIMARY, viewOver(heightOf(0)));
    made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(0)), true));
    // The view leaves and comes back: a new view to the pass.
    made.pass.step({ views: [], grounded: [] });
    made.pass.step({ views: [primary], grounded: [] });
    const returned = made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(1)), true));
    made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(2)), true));
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    stream(made, [primary]);
    const rebuilt = made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(3)), true));
    expect([
      onLattice(cascadesOf(returned).set, viewOver(heightOf(1)).cameraM),
      onLattice(cascadesOf(rebuilt).set, viewOver(heightOf(3)).cameraM),
    ]).toEqual([true, true]);
  });
});

describe("the fit's inputs (R10.T9.b; D6)", () => {
  /** A view's morph bands, as the pass makes them at τ. */
  function bands(view: TerrainView): Float64Array {
    const morph = new Float64Array(2 * (MAX_LEVEL + 1));
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      const [start, end] = morphRangeM(PLANET, level, {
        camera: { positionM: view.cameraM, orientation: view.orientation },
        fovXRad: view.fovXRad,
        viewport: view.viewport,
        weight: 1,
        tauPx: TERRAIN_SETTINGS.high.tauPx,
      });
      morph[2 * level] = start;
      morph[2 * level + 1] = end;
    }
    return morph;
  }

  it("takes the horizon texel continuous in h through each band and each split, from the level drawn at h", () => {
    const morph = bands(viewOver(50));
    const finest = PLANET.finestLevel;
    // From inside the finest level's reach out past level 3's band.
    const fromM = 0.5 * (morph[2 * finest + 1] ?? 0);
    const toM = 1.5 * (morph[2 * 3 + 1] ?? 0);
    let worst = 0;
    let previous = cascadeHorizonTexelM(PLANET, morph, fromM);
    for (let n = 1; n <= 20_000; n += 1) {
      const h = fromM * (toM / fromM) ** (n / 20_000);
      const texel = cascadeHorizonTexelM(PLANET, morph, h);
      worst = Math.max(worst, Math.abs(texel / previous - 1));
      previous = texel;
    }
    // Each step multiplies h by under 1.001 here, so the texel changes by under a few parts in a
    // thousand within a band, and never by a split's factor of 2.
    expect(worst).toBeLessThan(1e-2);
    // Each level's spacing doubled to its parent's at the band's end, and its own at its start.
    for (let level = 4; level <= finest; level += 1) {
      const start = morph[2 * level] ?? 0;
      const end = morph[2 * level + 1] ?? 0;
      const spacing = vertexSpacing(PLANET.figure.equatorialRadiusM, level).meanM;
      expect(cascadeHorizonTexelM(PLANET, morph, start) / spacing).toBeCloseTo(1, 12);
      expect(cascadeHorizonTexelM(PLANET, morph, end * (1 - 1e-12)) / spacing).toBeCloseTo(2, 6);
      expect(cascadeHorizonTexelM(PLANET, morph, end) / spacing).toBeCloseTo(2, 12);
    }
  });

  it("takes the level selection's traversal reaches from the root, past a fine level whose band end bumps beyond h", () => {
    const morph = bands(viewOver(50));
    const end = (level: number): number => morph[2 * level + 1] ?? Number.NaN;
    // Inside level 4's band, beyond level 5's end.
    const h = Math.sqrt(end(5) * end(4));
    const bumped = Float64Array.from(morph);
    const finest = PLANET.finestLevel;
    bumped[2 * finest + 1] = 10 * h;
    const spacing = vertexSpacing(PLANET.figure.equatorialRadiusM, 4).meanM;
    const texel = cascadeHorizonTexelM(PLANET, bumped, h);
    expect([finest > 5, texel, texel / spacing >= 1 && texel / spacing <= 2]).toEqual([
      true,
      cascadeHorizonTexelM(PLANET, morph, h),
      true,
    ]);
  });

  it("takes the coarsest level's spacing at any height beyond every band, and 0 for one below the ground", () => {
    const morph = bands(viewOver(50));
    const a = PLANET.figure.equatorialRadiusM;
    expect(cascadeHorizonTexelM(PLANET, morph, 1e9)).toBe(vertexSpacing(a, 0).meanM);
    expect(cascadeHorizonTexelM(PLANET, morph, -5)).toBe(cascadeHorizonTexelM(PLANET, morph, 0));
  });

  it("takes R as the figure's mean radius, (2a + c) ÷ 3", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = WGS84_FIGURE;
    expect(meanRadiusM(WGS84_FIGURE)).toBeCloseTo((2 * a + c) / 3, 6);
    // IUGG's R₁ for GRS 80, 6,371,008.7714 m (Moritz 1980), which WGS 84 shares to 0.1 mm.
    expect(meanRadiusM(WGS84_FIGURE)).toBeCloseTo(6_371_008.77, 1);
  });

  it("fits to the top edge of star 0's disc, its angular radius from the centre towards the zenith (decision-r10-t9b-penumbra)", async () => {
    const made = await streamedAt(NEAR);
    const { set } = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true)));
    const rho = STAR_RHO_RAD;
    expect([
      angleRad(set.basis.z, SUN),
      dot(set.basis.z, discTopDirection(SUN, FRAME_THERE.up, rho)),
      set.sunElevationRad,
    ]).toEqual([expect.closeTo(rho, 9), expect.closeTo(1, 12), expect.closeTo(15 * RAD + rho, 6)]);
    // The Sun's from 1 au: some 0.27°.
    expect(rho).toBeCloseTo(4.65e-3, 5);
  });

  it("fits as T9.a's fitCascades of the pass's own inputs, the device's depth-clip-control and the body's relief", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const { set } = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    // The ground is the datum's here: every bake is flat on it.
    const under = 50;
    const expected = fitCascades({
      cameraBodyFixedM: view.cameraM,
      orientationBodyFixed: view.orientation,
      rotation: IDENTITY_ROTATION,
      fovXRad: view.fovXRad,
      aspect: 640 / 360,
      heightAboveGroundM: under,
      sunDirectionBodyFixed: discTopDirection(SUN, FRAME_THERE.up, STAR_RHO_RAD),
      datumNormalBodyFixed: FRAME_THERE.up,
      bodyRadiusM: meanRadiusM(PLANET.figure),
      reliefRangeM: 8_000,
      horizonTexelM: 1,
      depthClipControl: made.engine.capabilities.depthClipControl,
    });
    expect([
      set.rangeM,
      set.sunElevationRad,
      set.casterReachM / expected.casterReachM,
      set.unclippedDepth,
      set.cascades[0].radiusM,
    ]).toEqual([
      expected.rangeM,
      expect.closeTo(expected.sunElevationRad, 6),
      expect.closeTo(1, 6),
      false,
      expect.closeTo(expected.cascades[0].radiusM, 6),
    ]);
  });

  it("takes no cascades, throwing nothing, for a viewport of no height, and drops the view's centres", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    const fit = (n: number, heightPx = 360): TerrainViewFrame => {
      const view = viewOver(heightOf(n));
      return made.pass.drawFor(
        PRIMARY,
        frameOf({ ...view, viewport: { widthPx: 640, heightPx } }, true, n),
      );
    };
    fit(0);
    const flat = fit(1, 0);
    const after = fit(2);
    expect([
      flat.cascades,
      flat.draws.length > 0,
      onLattice(cascadesOf(after).set, viewOver(heightOf(2)).cameraM),
    ]).toEqual([null, true, true]);
  });

  it("takes no cascades, throwing nothing, under a star of no direction", async () => {
    const made = await streamedAt(NEAR);
    const light = STARLIT.lights[0];
    if (light === undefined) {
      throw new Error("the cases' lighting has a star");
    }
    const frame = made.pass.drawFor(PRIMARY, {
      ...frameOf(viewOver(50), true),
      lighting: { ...STARLIT, lights: [{ ...light, direction: { x: 0, y: 0, z: 0 } }] },
    });
    expect([frame.cascades, made.engine.depthLayerRenders.length]).toEqual([null, 0]);
  });

  it("takes a relief that is not finite as 0, a caster reach of 0", async () => {
    const made = await litPass("high", undefined, () => Number.NaN);
    stream(made, [passView(PRIMARY, NEAR)]);
    const { set } = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true)));
    expect([set.casterReachM, set.weight > 0]).toEqual([0, true]);
  });
});

/** The angle between two directions, rad, by atan2 so that it keeps its precision when small. */
function angleRad(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

/** Fails a test where a pass it needs is missing. */
function fail(): never {
  throw new Error("a pass is missing");
}

/** `view` with its camera `moveM` north along the ground, its orientation kept. */
function movedNorth(view: TerrainView, moveM: number): TerrainView {
  return { ...view, cameraM: add(view.cameraM, scale(FRAME_THERE.north, moveM)) };
}

/** `view`, body-fixed as given, with the body turned `turnRad` about its pole. */
function bodyTurned(view: TerrainView, turnRad: number): TerrainView {
  const rows = rotationRows(quaternionFromAxisAngle({ x: 0, y: 0, z: 1 }, turnRad));
  const rotation = rotation3FromRows(rows);
  return {
    ...view,
    rotation,
    cameraM: rotateToBody(rotation, view.cameraM),
    orientation: multiply(quaternionFromRows(rows), view.orientation),
  };
}

/** A column-major 4 × 4 applied to (p, 1), in `f64`: clip x, y and depth. */
function applied(m: ArrayLike<number>, p: Vec3): Vec3 {
  const at = (i: number): number => m[i] ?? Number.NaN;
  return {
    x: at(0) * p.x + at(4) * p.y + at(8) * p.z + at(12),
    y: at(1) * p.x + at(5) * p.y + at(9) * p.z + at(13),
    z: at(2) * p.x + at(6) * p.y + at(10) * p.z + at(14),
  };
}

/** The layers a frame's caster passes render, nearest first; none for a frame without cascades. */
function layersOf(frame: TerrainViewFrame): number[] {
  return (frame.cascades?.passes ?? []).map((each) => each.layer);
}

describe("the far cascades at a lower rate (plan R14, R14.T6.b)", () => {
  // The camera a millimetre further north at each frame: within every hold's slack, and every
  // caster's record changed, so that T6.a reuses no cascade.
  let creep = 0;
  const crept = (view: TerrainView): TerrainView => {
    creep += 1;
    return movedNorth(view, creep * 1e-3);
  };

  it("renders cascades 2 and 3 on alternate halves of the high setting's period of 4, 4, 2, 3, 2, 3 passes over a creeping view", async () => {
    const made = await streamedAt(NEAR);
    const rendered = Array.from({ length: 5 }, () =>
      layersOf(made.pass.drawFor(PRIMARY, frameOf(crept(NEAR), true))),
    );
    expect([made.pass.geometry.farCascadeRate, rendered]).toEqual([
      4,
      [
        [0, 1, 2, 3],
        [0, 1],
        [0, 1, 3],
        [0, 1],
        [0, 1, 2],
      ],
    ]);
  });

  it("samples a held cascade as its own frame drew it, each body-fixed place at its map's same point after the camera moves and the body turns", async () => {
    const made = await streamedAt(NEAR);
    const first = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    // The Draw's fields are written again in place each frame.
    const drawnLight = Float32Array.from(drawOf(first).uniforms["cascadeLight2"] ?? []);
    const held = cascadesOf(first).set.cascades[2];
    const later = bodyTurned(movedNorth(NEAR, 3), 2e-5);
    const second = made.pass.drawFor(PRIMARY, frameOf(later, true));
    const sampled = drawOf(second).uniforms["cascadeLight2"] ?? new Float32Array(16);
    const fresh = cascadesOf(second).set.cascades[2].lightFromCamera;
    const cameraBefore = NEAR.cameraM;
    const cameraNow = movedNorth(NEAR, 3).cameraM;
    const places = [
      { x: 0, y: 0, z: 0 },
      { x: 120, y: -40, z: 25 },
      { x: -200, y: 90, z: -60 },
    ].map((offset) => add(held.centreBodyFixedM, offset));
    const apart = (light: ArrayLike<number>): number =>
      Math.max(
        ...places.map((place) => {
          const was = applied(drawnLight, sub(place, cameraBefore));
          const is = applied(light, rotateToBody(later.rotation, sub(place, cameraNow)));
          return Math.max(Math.abs(is.x - was.x), Math.abs(is.y - was.y), Math.abs(is.z - was.z));
        }),
      );
    // A hundredth of a texel is 10⁻⁵ in clip units; the fit's own cascade 2 now stands texels off.
    expect([
      layersOf(second),
      apart(sampled) < 1e-5,
      apart(fresh) > 1e-3,
      [...cascadesOf(second).received.cascades[2].lightFromCamera],
    ]).toEqual([[0, 1], true, true, [...sampled]]);
  });

  it("renders a held far cascade again once the camera moves beyond its slack, and holds the other", async () => {
    const made = await streamedAt(NEAR);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const near = layersOf(made.pass.drawFor(PRIMARY, frameOf(movedNorth(NEAR, 5), true)));
    made.pass.setGeometry(SETTINGS.high.geometry);
    made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    // Beyond cascade 2's 20.7 m at 50 m up, within cascade 3's 84 m.
    const far = layersOf(made.pass.drawFor(PRIMARY, frameOf(movedNorth(NEAR, 30), true)));
    expect([near, far]).toEqual([
      [0, 1],
      [0, 1, 2],
    ]);
  });

  /** The passes each of the primary's frames renders, drawn at `views` in turn. */
  const passCounts = (
    made: Made,
    views: ReadonlyArray<readonly [TerrainView, boolean]>,
  ): number[] =>
    views.map(
      ([view, takes]) => layersOf(made.pass.drawFor(PRIMARY, frameOf(crept(view), takes))).length,
    );
  const wider: TerrainView = { ...NEAR, viewport: { widthPx: 641, heightPx: 360 } };

  it.each([
    {
      cut: "a change of the geometry settings",
      between: (made: Made): void => {
        made.pass.setGeometry(SETTINGS.high.geometry);
      },
      after: [[NEAR, true]] as const,
      expected: [4],
    },
    { cut: "a resize", between: (): void => {}, after: [[wider, true]] as const, expected: [4] },
    {
      cut: "a frame without the cascades",
      between: (): void => {},
      after: [
        [NEAR, false],
        [NEAR, true],
      ] as const,
      expected: [0, 4],
    },
  ])("renders all four again after $cut", async ({ between, after, expected }) => {
    const made = await streamedAt(NEAR);
    const before = passCounts(made, [
      [NEAR, true],
      [NEAR, true],
    ]);
    between(made);
    expect([before, passCounts(made, after)]).toEqual([[4, 2], expected]);
  });

  it("renders all four again after a restore", async () => {
    const made = await streamedAt(NEAR);
    const before = passCounts(made, [
      [NEAR, true],
      [NEAR, true],
    ]);
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    stream(made, [passView(PRIMARY, NEAR)]);
    expect([before, passCounts(made, [[NEAR, true]])]).toEqual([[4, 2], [4]]);
  });

  it("renders all four every frame by the way back, whatever its rate", async () => {
    const made = await streamedAt(NEAR);
    made.pass.setGeometry({ ...SETTINGS.high.geometry, clusterPath: false });
    expect(
      passCounts(made, [
        [NEAR, true],
        [NEAR, true],
        [NEAR, true],
      ]),
    ).toEqual([4, 4, 4]);
  });

  it("renders every cascade at each view's frame where two views draw into the one set", async () => {
    const made = await litPass();
    const primary = viewOver(50, 0);
    const instrument = viewOver(80, 90);
    stream(made, [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)]);
    const rendered = Array.from({ length: 3 }, () => [
      layersOf(made.pass.drawFor(PRIMARY, frameOf(primary, true))),
      layersOf(made.pass.drawFor(INSTRUMENT, frameOf(instrument, true))),
    ]);
    expect(rendered.flat()).toEqual(Array.from({ length: 6 }, () => [0, 1, 2, 3]));
  });

  it("renders every cascade every frame while a caster that is not terrain casts, and at a rate of 1", async () => {
    const made = await streamedAt(NEAR);
    const remove = made.shadows.casters.add(() => []);
    const draw = (): number =>
      layersOf(made.pass.drawFor(PRIMARY, frameOf(crept(NEAR), true))).length;
    const withOther = [draw(), draw(), draw()];
    remove();
    const without = draw();
    made.pass.setGeometry(EVERY_FRAME);
    const everyFrame = [draw(), draw(), draw()];
    expect([withOther, without, everyFrame]).toEqual([[4, 4, 4], 2, [4, 4, 4]]);
  });

  it("culls no cluster into a held cascade, and widens the far two's receivers by their slack with no cover below (R14.T1.c)", async () => {
    const made = await clusteredPass();
    const lastCull = (): CascadeCullPass[] =>
      parsePasses(
        made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL)?.bindings.uniforms[
          "passes"
        ] ?? new Uint32Array(),
      );
    const first = made.pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const rendered = lastCull();
    // A millimetre on, so that the near two are not reused (R14.T6.a).
    made.pass.drawFor(PRIMARY, frameOf(movedNorth(NEAR, 1e-3), true));
    const held = lastCull();
    const [, , c2, c3] = cascadesOf(first).set.cascades;
    expect([
      rendered.map((pass) => pass.receivers?.coveredBelow === null),
      rendered.slice(2).map((pass) => pass.receivers?.depthM[1]),
      held.length,
    ]).toEqual([
      [true, false, true, true],
      [Math.fround(c2.farM + c2.holdSlackM), Math.fround(c3.farM + c3.holdSlackM)],
      2,
    ]);
  });
});

describe("the cascades re-rendered only when they change (plan R14, R14.T6.a)", () => {
  /** A pass streamed at {@link NEAR}, every cascade due each frame (T6.b's rate 1). */
  async function everyFrame(): Promise<Made> {
    const made = await streamedAt(NEAR);
    made.pass.setGeometry(EVERY_FRAME);
    return made;
  }

  /** {@link everyFrame}'s, streamed until nothing more is asked for, so that a step adds none. */
  async function settled(): Promise<Made> {
    const made = await litPass();
    stream(made, [passView(PRIMARY, NEAR)], 24);
    made.pass.setGeometry(EVERY_FRAME);
    return made;
  }

  /** The layers each of the primary's frames renders, at `inputs` in turn. */
  const rendersOf = (made: Made, inputs: ReadonlyArray<TerrainViewFrameInput>): number[][] =>
    inputs.map((input) => layersOf(made.pass.drawFor(PRIMARY, input)));

  /** {@link STARLIT} with star 0 turned `turnRad` about the place's north. */
  function sunTurned(turnRad: number): TerrainViewFrameInput {
    const [star] = STARLIT.lights;
    if (star === undefined) {
      throw new Error("the test's lighting has no star");
    }
    const direction = add(
      scale(SUN, Math.cos(turnRad)),
      scale(cross(FRAME_THERE.north, SUN), Math.sin(turnRad)),
    );
    return {
      ...frameOf(NEAR, true),
      lighting: { ...STARLIT, lights: [{ ...star, direction }] },
    };
  }

  it("renders all four at the first frame of a still camera with time paused, and none after", async () => {
    const made = await everyFrame();
    expect(
      rendersOf(
        made,
        [0, 1, 2].map(() => frameOf(NEAR, true)),
      ),
    ).toEqual([[0, 1, 2, 3], [], []]);
  });

  it.each([
    {
      input: "the camera's place",
      toggled: (): TerrainViewFrameInput => frameOf(movedNorth(NEAR, 1e-3), true),
    },
    {
      input: "the view's axis",
      toggled: (): TerrainViewFrameInput => frameOf(viewOver(50, 1e-3), true),
    },
    {
      input: "the body's rotation",
      toggled: (): TerrainViewFrameInput => frameOf(bodyTurned(NEAR, 1e-9), true),
    },
    { input: "the sun's direction", toggled: (): TerrainViewFrameInput => sunTurned(1e-6) },
  ])(
    "renders every cascade once again where $input changes, and none after",
    async ({ toggled }) => {
      const made = await everyFrame();
      const still = frameOf(NEAR, true);
      expect(rendersOf(made, [still, still, toggled(), toggled()])).toEqual([
        [0, 1, 2, 3],
        [],
        [0, 1, 2, 3],
        [],
      ]);
    },
  );

  it("reuses the interior patches' casters on the cluster path as the others', and renders them again where the camera moves", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(EVERY_FRAME);
    const still = frameOf(NEAR, true);
    const moved = frameOf(movedNorth(NEAR, 1e-3), true);
    const rendered = rendersOf(made, [still, still, moved, moved]);
    const clustered = cascadesOf(made.pass.drawFor(PRIMARY, moved)).clustered;
    expect([clustered, rendered]).toEqual([true, [[0, 1, 2, 3], [], [0, 1, 2, 3], []]]);
  });

  it("culls no cluster into a reused cascade", async () => {
    const made = await clusteredPass();
    made.pass.setGeometry(EVERY_FRAME);
    const culled = (): number =>
      parsePasses(
        made.engine.dispatched.findLast((d) => d.kernel === LIT_CULL)?.bindings.uniforms[
          "passes"
        ] ?? new Uint32Array(),
      ).length;
    const still = frameOf(NEAR, true);
    made.pass.drawFor(PRIMARY, still);
    const first = culled();
    const dispatches = made.engine.dispatched.length;
    made.pass.drawFor(PRIMARY, still);
    expect([first, made.engine.dispatched.length > dispatches, culled()]).toEqual([4, true, 0]);
  });

  it("counts none of a reused cascade's lit-mesh casters", async () => {
    const made = await everyFrame();
    const still = frameOf(NEAR, true);
    const first = cascadesOf(made.pass.drawFor(PRIMARY, still)).litMeshCasters;
    const again = cascadesOf(made.pass.drawFor(PRIMARY, still)).litMeshCasters;
    expect([first.every((count) => count > 0), again]).toEqual([true, [0, 0, 0, 0]]);
  });

  it.each([
    {
      setting: "casterDepthCull",
      toggled: { ...EVERY_FRAME, casterDepthCull: !EVERY_FRAME.casterDepthCull },
    },
    {
      setting: "casterStrideCap",
      toggled: { ...EVERY_FRAME, casterStrideCap: EVERY_FRAME.casterStrideCap === 1 ? 2 : 1 },
    },
  ] as const)("renders again once where $setting changes, and none after", async ({ toggled }) => {
    const made = await everyFrame();
    const still = frameOf(NEAR, true);
    const before = rendersOf(made, [still, still]);
    made.pass.setGeometry(toggled);
    expect([before, rendersOf(made, [still, still])]).toEqual([
      [[0, 1, 2, 3], []],
      [[0, 1, 2, 3], []],
    ]);
  });

  it("renders again once where a grounded contact moves its casters' hold, and none after", async () => {
    const made = await settled();
    const still = frameOf(NEAR, true);
    const before = rendersOf(made, [still, still]);
    const [x, y, z] = surfacePoint(PLANET.figure, OVER, 0);
    const views = [passView(PRIMARY, NEAR)];
    const anyOf = (): boolean[] => rendersOf(made, [still, still]).map((each) => each.length > 0);
    made.pass.step({ views, grounded: [] });
    const stepped = anyOf();
    made.pass.step({ views, grounded: [{ positionM: { x, y, z }, radiusM: 10 }] });
    expect([before, stepped, anyOf()]).toEqual([
      [[0, 1, 2, 3], []],
      [false, false],
      [true, false],
    ]);
  });

  it("renders every frame while a caster with no revision casts, and once more where one with a revision changes it", async () => {
    const made = await everyFrame();
    const still = frameOf(NEAR, true);
    const unrevised = made.shadows.casters.add(() => []);
    const without = rendersOf(made, [still, still]);
    unrevised();
    let revision = 0;
    made.shadows.casters.add(
      () => [],
      () => revision,
    );
    const steady = rendersOf(made, [still, still]);
    revision += 1;
    const changed = rendersOf(made, [still, still]);
    expect([without, steady, changed]).toEqual([
      [
        [0, 1, 2, 3],
        [0, 1, 2, 3],
      ],
      [[0, 1, 2, 3], []],
      [[0, 1, 2, 3], []],
    ]);
  });

  it("renders again where another view drew into the set between, and by the way back every frame", async () => {
    const made = await litPass();
    const instrument = viewOver(80, 90);
    stream(made, [passView(PRIMARY, NEAR), passView(INSTRUMENT, instrument)]);
    made.pass.setGeometry(EVERY_FRAME);
    const still = frameOf(NEAR, true);
    const first = rendersOf(made, [still, still]);
    made.pass.drawFor(INSTRUMENT, frameOf(instrument, true));
    const after = rendersOf(made, [still, still]);
    made.pass.setGeometry({ ...EVERY_FRAME, clusterPath: false });
    const wayBack = rendersOf(made, [still, still]);
    made.pass.setGeometry(EVERY_FRAME);
    const onThePath = rendersOf(made, [still, still]);
    expect([first, after, wayBack, onThePath]).toEqual([
      [[0, 1, 2, 3], []],
      [[0, 1, 2, 3], []],
      [
        [0, 1, 2, 3],
        [0, 1, 2, 3],
      ],
      [[0, 1, 2, 3], []],
    ]);
  });

  it("renders again where the draw set's keys change, a landed bake drawn, and none after", async () => {
    const made = await everyFrame();
    const still = frameOf(NEAR, true);
    const before = rendersOf(made, [still, still]);
    const keysOf = (): string =>
      (made.pass.lastStep?.drawSet.patches ?? []).map((entry) => entry.keyString).join();
    const keys = keysOf();
    made.lane().bakeDemand(made.pass);
    made.pass.step({ views: [passView(PRIMARY, NEAR)], grounded: [] });
    const after = rendersOf(made, [still, still]).map((each) => each.length > 0);
    expect([before, keysOf() !== keys, after]).toEqual([[[0, 1, 2, 3], []], true, [true, false]]);
  });

  it("renders again where a drawn patch is baked again, and none after", async () => {
    const made = await settled();
    made.pass.setReducedMotion(true);
    const still = frameOf(NEAR, true);
    const before = rendersOf(made, [still, still]);
    const [drawn] = made.pass.lastStep?.drawSet.patches ?? [];
    if (drawn === undefined) {
      throw new Error("nothing is drawn");
    }
    const views = [passView(PRIMARY, NEAR)];
    const anyOf = (): boolean[] => rendersOf(made, [still, still]).map((each) => each.length > 0);
    made.pass.step({ views, grounded: [] });
    const stepped = anyOf();
    made.lane().deliver(made.pass, drawn.patch.key);
    made.pass.step({ views, grounded: [] });
    expect([before, stepped, anyOf()]).toEqual([
      [[0, 1, 2, 3], []],
      [false, false],
      [true, false],
    ]);
  });

  it("renders every frame a fade of the drawn ground runs, its morph fields between 0 and 1, and none once it ends", async () => {
    const made = await litPass();
    // Streamed from far above and then low: the refines' fades begin at the first draw.
    stream(made, [passView(PRIMARY, viewOver(5_000))]);
    stream(made, [passView(PRIMARY, NEAR)]);
    made.pass.setGeometry(EVERY_FRAME);
    const rendered = Array.from({ length: 60 }, (_, n) => {
      const frame = made.pass.drawFor(PRIMARY, frameOf(NEAR, true, 100 * n));
      let fading = false;
      // The lit mesh's records and the cluster path's, an edge bake's stand-ins among them
      // (R14.T3.c's edge stand-ins).
      for (const buffer of [
        `terrain instances (${PRIMARY})`,
        `terrain interior instances (${PRIMARY})`,
      ]) {
        const data = made.engine.writes.findLast((w) => w.buffer === buffer)?.data;
        const words = new Uint32Array(data?.slice().buffer ?? new ArrayBuffer(0));
        for (let at = INSTANCE_FADES_WORD; at < words.length; at += INSTANCE_RECORD_BYTES / 4) {
          const own = (words[at] ?? 0) & 0xff;
          fading ||= own > 0 && own < 255;
        }
      }
      return { layers: layersOf(frame).length, fading };
    });
    const whileFading = rendered.filter((frame) => frame.fading);
    expect([
      whileFading.length > 0,
      whileFading.every((frame) => frame.layers > 0),
      rendered.slice(-2).map((frame) => [frame.fading, frame.layers]),
    ]).toEqual([
      true,
      true,
      [
        [false, 0],
        [false, 0],
      ],
    ]);
  });
});

describe("the cascades' passes over a descent, with time stopped and running (plan R14, R14.T6.a)", () => {
  /** R05's descent's 300 m/s at 60 Hz, m a frame, and the body's turn a frame: a day of 24 h. */
  const STEP_M = 5;
  const TURN_RAD = (2 * Math.PI) / 86_400 / 60;

  /** The passes each frame renders: `frames` descending, then `frames` hovering at the bottom. */
  async function passesOver(timeRuns: boolean, frames: number): Promise<number[]> {
    const made = await streamedAt(NEAR);
    made.pass.setGeometry(EVERY_FRAME);
    const counts: number[] = [];
    for (let n = 0; n < 2 * frames; n += 1) {
      const descended = Math.min(n, frames - 1) * STEP_M;
      const at = movedNorth(viewOver(50 + (frames - 1) * STEP_M - descended), descended);
      const view = timeRuns ? bodyTurned(at, n * TURN_RAD) : at;
      counts.push(layersOf(made.pass.drawFor(PRIMARY, frameOf(view, true))).length);
    }
    return counts;
  }

  it("renders every cascade every frame of the descent, and none of a hover with time stopped, but every frame with it running", async () => {
    const stopped = await passesOver(false, 4);
    const running = await passesOver(true, 4);
    expect([stopped, running]).toEqual([
      [4, 4, 4, 4, 0, 0, 0, 0],
      [4, 4, 4, 4, 4, 4, 4, 4],
    ]);
  });
});

/**
 * The slots of 0 to 3 whose upload re-renders the cascade of a record in slot 1 with `parent` and
 * `fades` (plan R14, R14.T6.a; `appendCasterRecord`).
 */
function slotsRead(parent: number, fades: number): number[] {
  return [0, 1, 2, 3].filter((slot) => {
    const words = new Uint32Array(INSTANCE_RECORD_BYTES / 4);
    words[INSTANCE_SLOT_WORD] = 1;
    words[INSTANCE_PARENT_SLOT_WORD] = parent;
    words[INSTANCE_FADES_WORD] = fades;
    const versions = new Uint32Array(4);
    const reuse = new CascadeReuse();
    reuse.start();
    appendCasterRecord(reuse, words, 0, versions);
    reuse.rendered(0);
    versions[slot] = 1;
    reuse.start();
    appendCasterRecord(reuse, words, 0, versions);
    return !reuse.unchanged(0);
  });
}

describe("a cascade's caster inputs (plan R14, R14.T6.a)", () => {
  it("is unchanged where its words equal its last render's, one for one, and changed by any word", () => {
    const reuse = new CascadeReuse();
    const write = (words: number[]): void => {
      reuse.start();
      reuse.words(Uint32Array.from(words));
    };
    write([1, 2, 3]);
    const before = reuse.unchanged(0);
    reuse.rendered(0);
    write([1, 2, 3]);
    const same = reuse.unchanged(0);
    const other = reuse.unchanged(1);
    write([1, 2, 4]);
    const changed = reuse.unchanged(0);
    write([1, 2, 3, 0]);
    const longer = reuse.unchanged(0);
    reuse.forget();
    write([1, 2, 3]);
    expect([before, same, other, changed, longer, reuse.unchanged(0)]).toEqual([
      false,
      true,
      false,
      false,
      false,
      false,
    ]);
  });

  it("numbers the inputs the cascades share anew where they change, and as before where they do not", () => {
    const reuse = new CascadeReuse();
    const shared = (word: number): number => {
      reuse.start();
      reuse.word(word);
      return reuse.shared();
    };
    const first = shared(7);
    const same = shared(7);
    const changed = shared(8);
    const back = shared(7);
    expect([same === first, changed === first, back === first, back === changed]).toEqual([
      true,
      false,
      false,
      false,
    ]);
  });

  it("re-renders a record's cascade where a slot its caster reads is uploaded again: its own, and while its heights mix the older bake's or its parent's", () => {
    const half = mixStep(0.5);
    expect([
      slotsRead(2, fadeWord(128, 0, 0)),
      slotsRead(2, fadeWord(0, half, 3)),
      slotsRead(2, fadeWord(0, half, PARENT_MIX_SLOT)),
      slotsRead(NO_PARENT_SLOT, fadeWord(0, half, PARENT_MIX_SLOT)),
      slotsRead(2, fadeWord(0, half, DATUM_SLOT)),
    ]).toEqual([[1], [1, 3], [1, 2], [1], [1]]);
  });

  it("takes an f64 as its two words, so that a change past f32's precision counts", () => {
    const reuse = new CascadeReuse();
    reuse.start();
    reuse.f64(6_378_137.000_001);
    reuse.rendered(0);
    reuse.start();
    reuse.f64(6_378_137.000_002);
    expect(reuse.unchanged(0)).toBe(false);
  });
});
