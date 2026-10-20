/**
 * The terrain pass in the wireframe's shading (plan R10, R10.T11): a body baked without its class
 * weights, drawn as depth with its contours and graticule, its material compiled for the canvas,
 * its draw's uniforms and storage, the phase record it writes for each drawn patch, and its
 * annunciation and mask.
 */

import { describe, expect, it } from "vitest";

import { PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { type Vec3, vec3 } from "../../geometry/vec3";
import { CountingRenderEngine, fakeDevice } from "../../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION, type Rotation3, rotation3FromRows } from "../coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import type {
  MaterialHandle,
  MeshHandle,
  RenderTargetFormat,
  WgslMaterialSpec,
} from "../engine/types";
import {
  type QualitySetting,
  TERRAIN_SETTINGS,
  WIREFRAME_TERRAIN_TAU_PX,
} from "../quality/qualitySetting";
import { vertexDir } from "./cube";
import { finestPatchSizeM, heldRadiusM, morphRampM } from "./grounded";
import { INSTANCE_RECORD_BYTES, WIRE_PHASE_RECORD_BYTES } from "./gpu/uniforms";
import { type PatchKey, patchKeyString } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { PRIMARY_VIEW_WEIGHT } from "./priority";
import type { PatchRequest } from "./select";
import { PATCH_VERTICES_PER_SIDE } from "./slotLayout";
import {
  TerrainPass,
  type TerrainPassView,
  type TerrainPool,
  type TerrainView,
  type TerrainViewFrameInput,
  type TerrainWireFrameInput,
} from "./terrainPass";
import {
  contourPhase,
  graticulePatch,
  terrainWireMaterialSpec,
  wireframeTerrainSettings,
} from "./terrainWire";
import {
  type BakedPatch,
  type BakeSettings,
  type HeightSourceSetting,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

const PRIMARY = PRIMARY_VIEW_ID;

/** A body baked without its class weights: the wireframe's (decision D10). */
const WIRE_BODY = {
  kind: "body",
  body: "0200080020000000.0301",
  detailSeed: "617373656d626c79",
  weights: "skipped",
} as const;

/** The same body baked with its weights: the photorealistic style's. */
const LIT_BODY = { ...WIRE_BODY, weights: "baked" } as const;

const STEP_RAD = (10 * Math.PI) / 180;

/** A wireframe frame's contours and graticule, every value distinct. */
const WIRE: TerrainWireFrameInput = {
  intervalM: 200,
  seaLevelM: -150.25,
  strokeScale: 2,
  graticule: { stepRad: STEP_RAD, thinPx: 2, heavyPx: 3, casingPx: 1 },
  colours: {
    muted: Float32Array.of(0.25, 0.26, 0.27, 1),
    text: Float32Array.of(0.8, 0.81, 0.82, 1),
    casing: Float32Array.of(0.002, 0.003, 0.004, 1),
  },
  occluderSlopePx: 5,
};

/** A bake's layers as the wireframe's are baked: no weights and no horizon map. */
function wireLayers(fieldLevel = 8, unsurveyed: ReadonlyArray<number> = []): TerrainLayers {
  return {
    palette: new Uint8Array(16).fill(PALETTE_NONE),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(65 * 65).fill(unsurveyed.length === 0 ? 255 : 0),
    fieldLevel,
    boundM: 1,
    sigmaM: 0,
    complete: unsurveyed.length === 0,
    unsurveyedCells: Uint32Array.from(unsurveyed),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: false,
    horizonRefusedCells: new Uint32Array(),
  };
}

/** The origin of `key`'s bake: its centre vertex on the datum, body-fixed, m. */
function originOf(key: PatchKey): Vec3 {
  const [x, y, z] = surfacePoint(PLANET.figure, vertexDir(key, 32, 32), 0);
  return vec3(x, y, z);
}

/** The origin height of `key`'s bake, m: distinct for every key, fractional, past `f32`'s step. */
function originHeightOf(key: PatchKey): number {
  return 12_345.678_9 + 1_009.1 * key.level + 37.7 * key.i + 11.3 * key.j + 101.9 * key.face;
}

/** A lane that records the demand and bakes on the test's command, each bake at its own origin. */
class WireLane implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  /** The layers each bake carries unless its key has its own. */
  layers: TerrainLayers = wireLayers();
  readonly layersOf = new Map<string, TerrainLayers>();
  readonly #baked = new Set<(bake: BakedPatch) => void>();

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

  terminate(): void {}

  /** Answers every demanded patch; returns how many. */
  bakeDemand(pass: TerrainPass): number {
    const demand = this.demand;
    for (const request of demand) {
      this.deliver(pass, request.key);
    }
    return demand.length;
  }

  /** Bakes `key` at its own origin and origin height, or at `at`'s where given. */
  deliver(
    pass: TerrainPass,
    key: PatchKey,
    at: { readonly originM: Vec3; readonly originHeightM: number } = {
      originM: originOf(key),
      originHeightM: originHeightOf(key),
    },
  ): void {
    // The wireframe keeps no normals; a lit layout takes them at its atlas's resolution.
    const n = pass.layout.atlas?.samplesPerSide ?? PATCH_VERTICES_PER_SIDE;
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: at.originM,
      heights: new Float32Array(65 * 65 * 2),
      offsets: this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
      normals: new Float16Array(n * n * 2),
      heightRangeM: [0, 0],
      boundingRadiusM: 1,
      originHeightM: at.originHeightM,
      skirtDepthM: 1,
      layers: this.layersOf.get(patchKeyString(key)) ?? this.layers,
    };
    for (const listener of this.#baked) {
      listener(bake);
    }
  }
}

/** One material compiled through `createMaterialAsync`, with its targets and meshes. */
interface Compiled {
  readonly spec: WgslMaterialSpec;
  readonly targets: ReadonlyArray<RenderTargetFormat>;
  readonly meshes: ReadonlyArray<string>;
}

/** A counting engine that also records what each material is compiled for. */
class CompileRecordingEngine extends CountingRenderEngine {
  readonly compiled: Compiled[] = [];

  override createMaterialAsync(
    spec: WgslMaterialSpec,
    targets: ReadonlyArray<RenderTargetFormat> = [],
    meshes: ReadonlyArray<MeshHandle> = [],
  ): Promise<MaterialHandle> {
    this.compiled.push({ spec, targets, meshes: meshes.map((mesh) => mesh.name) });
    return super.createMaterialAsync(spec);
  }
}

interface Made {
  readonly pass: TerrainPass;
  readonly engine: CompileRecordingEngine;
  /** The pass's current lane. */
  readonly lane: () => WireLane;
}

/**
 * A pass over the body on `setting`, ready: the wireframe's, with the wireframe's cache as the
 * session makes it, or the lit one with the setting's own.
 */
async function passOver(setting: QualitySetting, source: HeightSourceSetting): Promise<Made> {
  const engine = new CompileRecordingEngine(
    await fakeDevice({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    }),
  );
  const lanes: WireLane[] = [];
  const wire = source.kind === "body" && source.weights === "skipped";
  const pass = new TerrainPass({
    engine,
    setting,
    planet: PLANET,
    source,
    ...(wire ? { terrain: wireframeTerrainSettings(setting) } : {}),
    createPool: (bake) => {
      const lane = new WireLane(bake);
      lanes.push(lane);
      return lane;
    },
  });
  await pass.ready();
  return {
    pass,
    engine,
    lane: () => {
      const lane = lanes.at(-1);
      if (lane === undefined) {
        throw new Error("no lane was taken");
      }
      return lane;
    },
  };
}

/** The wireframe's pass over the body on `setting`, ready. */
function wirePass(setting: QualitySetting = "low"): Promise<Made> {
  return passOver(setting, WIRE_BODY);
}

/** A camera `heightM` above the north pole, looking straight down, the body turned by `rotation`. */
function northPole(heightM: number, rotation: Rotation3 = IDENTITY_ROTATION): TerrainView {
  return {
    rotation,
    cameraM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + heightM },
    orientation: lookAlong({ x: 0, y: 0, z: -1 }, { x: 1, y: 0, z: 0 }),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 640, heightPx: 360 },
  };
}

const VIEW = northPole(2_000_000);

/** The primary view of the pass at `tauPx`, the wireframe's 4 px by default. */
function passView(view: TerrainView, tauPx = WIREFRAME_TERRAIN_TAU_PX): TerrainPassView {
  return { id: PRIMARY, view, tauPx, weight: PRIMARY_VIEW_WEIGHT };
}

/** A view's frame inputs, with the wireframe's contours where given. */
function frameOf(
  view: TerrainView,
  wire: TerrainWireFrameInput | undefined,
  nowMs = 0,
): TerrainViewFrameInput {
  return {
    view,
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    ...(wire === undefined ? {} : { wire }),
    nowMs,
  };
}

/** Steps the pass over the primary and bakes its demand, `rounds` times, then steps once more. */
function stream(made: Made, rounds: number, view: TerrainPassView = passView(VIEW)): void {
  for (let n = 0; n < rounds; n += 1) {
    made.pass.step({ views: [view], grounded: [] });
    made.lane().bakeDemand(made.pass);
  }
  made.pass.step({ views: [view], grounded: [] });
}

/** Steps and bakes until nothing the primary sees stands in or is missing, at most 40 rounds. */
function streamed(made: Made, view: TerrainPassView = passView(VIEW)): void {
  made.pass.step({ views: [view], grounded: [] });
  for (let round = 0; round < 40; round += 1) {
    if (made.lane().bakeDemand(made.pass) === 0) {
      break;
    }
    made.pass.step({ views: [view], grounded: [] });
  }
}

/** The bytes last written to the buffer named `name`. */
function lastWrite(engine: CountingRenderEngine, name: string): ArrayBuffer {
  return (engine.writes.findLast((w) => w.buffer === name)?.data ?? new Uint8Array()).slice()
    .buffer;
}

/** The slots of the instance records last written for the primary, in order. */
function instanceSlots(engine: CountingRenderEngine): number[] {
  const u = new Uint32Array(lastWrite(engine, `terrain instances (${PRIMARY})`));
  const slots: number[] = [];
  for (let n = 0; n < u.length / (INSTANCE_RECORD_BYTES / 4); n += 1) {
    slots.push(u[n * 8 + 3] ?? -1);
  }
  return slots;
}

/** The phase records last written for the primary, twelve `f32` each, in order. */
function phaseRecords(engine: CountingRenderEngine): number[][] {
  const f = new Float32Array(lastWrite(engine, `terrain phases (${PRIMARY})`));
  const words = WIRE_PHASE_RECORD_BYTES / 4;
  const records: number[][] = [];
  for (let n = 0; n < f.length / words; n += 1) {
    records.push([...f.subarray(n * words, (n + 1) * words)]);
  }
  return records;
}

/** The record a patch of origin `originM` and origin height `originHeightM` takes under `wire`. */
function phaseOf(originHeightM: number, originM: Vec3, wire: TerrainWireFrameInput): number[] {
  const h0 = Math.fround(originHeightM);
  const place = graticulePatch(originM, wire.graticule?.stepRad ?? 0);
  return [
    contourPhase(h0, wire.intervalM),
    wire.seaLevelM === null ? 0 : h0 - wire.seaLevelM,
    place.radiusM,
    0,
    place.latPhaseRad,
    place.lonPhaseRad,
    place.latIndex,
    place.lonIndex,
    place.sinLat,
    place.cosLat,
    place.sinLon,
    place.cosLon,
  ].map(Math.fround);
}

/**
 * The records the primary's instances should carry under `wire`, in their order: each drawn
 * patch's from its own bake (`at` for the keys a test baked again).
 */
function expectedPhases(
  made: Made,
  wire: TerrainWireFrameInput,
  at: ReadonlyMap<string, { readonly originM: Vec3; readonly originHeightM: number }> = new Map(),
): number[][] {
  const drawn = new Map(
    (made.pass.lastStep?.drawSet.patches ?? []).map((patch) => [patch.patch.slot, patch]),
  );
  return instanceSlots(made.engine).map((slot) => {
    const patch = drawn.get(slot);
    if (patch === undefined) {
      return [];
    }
    const baked = at.get(patch.keyString);
    return phaseOf(
      baked?.originHeightM ?? originHeightOf(patch.patch.key),
      baked?.originM ?? originOf(patch.patch.key),
      wire,
    );
  });
}

describe("the wireframe's terrain pass (R10.T11)", () => {
  it("compiles the wireframe's material for the canvas over the lit material's mesh, and nothing more", async () => {
    const { pass, engine } = await wirePass();
    expect(engine.compiled).toEqual([
      {
        spec: terrainWireMaterialSpec(pass.layout.vertexPath),
        targets: ["canvas"],
        meshes: ["terrain lit patch"],
      },
    ]);
  });

  it("makes no depth material for the symbology, which draws over its own depth", async () => {
    const { pass } = await wirePass();
    expect(pass.symbologyDepthMaterial).toBeNull();
  });

  it("draws its patches as one indirect draw of the wireframe's material over the lit mesh", async () => {
    const made = await wirePass();
    streamed(made);
    const draw = made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).draw;
    expect([
      draw?.mesh.name,
      draw?.material.name,
      draw?.textures,
      draw?.indirect?.buffer.name,
      draw?.indirect?.offsetBytes,
    ]).toEqual([
      "terrain lit patch",
      "terrain-wire-face-differences",
      {},
      `terrain indirect (${PRIMARY})`,
      0,
    ]);
  });

  it("binds R05's vertex storage, the survey masks and the view's phases", async () => {
    const made = await wirePass();
    streamed(made);
    const draw = made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).draw;
    const bound = Object.fromEntries(
      Object.entries(draw?.storageBuffers ?? {}).map(([name, buffer]) => [name, buffer.name]),
    );
    expect(bound).toEqual({
      contacts: `terrain contacts (${PRIMARY})`,
      heights: "terrain heights",
      instances: `terrain instances (${PRIMARY})`,
      slots: "terrain slot records",
      surveyMask: "terrain survey-mask",
      wirePhases: `terrain phases (${PRIMARY})`,
    });
  });

  it("binds the offsets too on the baked-offsets path", async () => {
    const made = await wirePass("high");
    streamed(made);
    const draw = made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).draw;
    expect([made.pass.layout.vertexPath, draw?.storageBuffers?.["offsets"]?.name]).toEqual([
      "baked-offsets",
      "terrain offsets",
    ]);
  });

  it("gives its draw the interval, stroke scale, coastline, widths, graticule, colours and slope term", async () => {
    const made = await wirePass();
    streamed(made);
    const draw = made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).draw;
    const uniforms = Object.fromEntries(
      Object.entries(draw?.uniforms ?? {}).map(([name, value]) => [name, [...value]]),
    );
    expect(uniforms).toEqual({
      bodyRotation: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      // Zeros: the wireframe reads no normals.
      atlas: [0, 0, 0, 0],
      contour: [200, 2, 1, Math.fround(1 / 200)],
      // Minor 1 px, index 1.5 px and the coastline 2 px, each at the stroke scale of 2.
      contourWidthsPx: [2, 3, 4, 0],
      graticule: [Math.fround(STEP_RAD), 2, 3, 1],
      muted: [...WIRE.colours.muted],
      text: [...WIRE.colours.text],
      casing: [...WIRE.colours.casing],
      occluderSlopePx: [5],
    });
  });

  it("draws no coastline and no graticule where the world has no sea and the view no graticule", async () => {
    const made = await wirePass();
    streamed(made);
    const draw = made.pass.drawFor(
      PRIMARY,
      frameOf(VIEW, { ...WIRE, seaLevelM: null, graticule: null }),
    ).draw;
    expect([
      [...(draw?.uniforms["contour"] ?? [])],
      [...(draw?.uniforms["graticule"] ?? [])],
    ]).toEqual([
      [200, 2, 0, Math.fround(1 / 200)],
      [0, 0, 0, 0],
    ]);
  });

  it("draws with the body's rotation into the view's axes, column-major", async () => {
    // A quarter turn about the pole, which leaves the camera over the pole.
    const turned = northPole(
      2_000_000,
      rotation3FromRows([
        { x: 0, y: -1, z: 0 },
        { x: 1, y: 0, z: 0 },
        { x: 0, y: 0, z: 1 },
      ]),
    );
    const made = await wirePass();
    streamed(made, passView(turned));
    const draw = made.pass.drawFor(PRIMARY, frameOf(turned, WIRE)).draw;
    expect([...(draw?.uniforms["bodyRotation"] ?? [])]).toEqual([
      0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
    ]);
  });

  it("draws nothing in a frame given no contours to draw, though it sees its patches", async () => {
    const made = await wirePass();
    streamed(made);
    const frame = made.pass.drawFor(PRIMARY, frameOf(VIEW, undefined));
    expect([frame.draw, made.pass.seesPatch(PRIMARY)]).toEqual([null, true]);
  });

  it("rewrites one draw's uniforms in place from frame to frame", async () => {
    const made = await wirePass();
    streamed(made);
    const first = made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).draw;
    const second = made.pass.drawFor(PRIMARY, frameOf(VIEW, { ...WIRE, intervalM: 500 })).draw;
    expect([second === first, second?.uniforms["contour"]?.[0]]).toEqual([true, 500]);
  });

  it("writes a grounded craft's contact for its draw, so that the ground under it holds its morph at zero", async () => {
    const made = await wirePass();
    const contact = { positionM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM }, radiusM: 10 };
    // Over the craft, 2 km up: the view sees the forced region under it.
    const over = northPole(2_000);
    const view = passView(over);
    made.pass.step({ views: [view], grounded: [contact] });
    for (let round = 0; round < 60 && made.lane().bakeDemand(made.pass) > 0; round += 1) {
      made.pass.step({ views: [view], grounded: [contact] });
    }
    made.pass.drawFor(PRIMARY, frameOf(over, WIRE));
    const f = new Float32Array(lastWrite(made.engine, `terrain contacts (${PRIMARY})`));
    const u = new Uint32Array(f.buffer);
    const patchM = finestPatchSizeM(PLANET);
    // The contact less the camera, its held radius and its ramp (Design note 6).
    expect([u[0], ...f.subarray(4, 9)]).toEqual([
      1,
      0,
      0,
      Math.fround(PLANET.figure.polarRadiusM - over.cameraM.z),
      Math.fround(heldRadiusM(contact, patchM)),
      Math.fround(morphRampM(patchM)),
    ]);
  });

  it("makes no GPU object after warm-up, frame after frame", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    made.engine.resetCounts();
    for (let n = 1; n < 20; n += 1) {
      made.pass.step({ views: [passView(VIEW)], grounded: [] });
      made.pass.drawFor(PRIMARY, frameOf(VIEW, { ...WIRE, intervalM: n % 2 === 0 ? 200 : 500 }));
    }
    const { buffers, textures, meshes, materials } = made.engine.counts;
    expect([buffers, textures, meshes, materials]).toEqual([0, 0, 0, 0]);
  });
});

describe("the wireframe's phase records (R10.T11)", () => {
  it("writes one record a drawn patch in its instances' order: its contour phase, height above the sea and graticule place", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    const records = phaseRecords(made.engine);
    expect([records.length > 1, records]).toEqual([true, expectedPhases(made, WIRE)]);
  });

  it("forms each patch's contour phase from its bake's origin height as the slot record narrows it", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    const drawn = new Map(
      (made.pass.lastStep?.drawSet.patches ?? []).map((patch) => [patch.patch.slot, patch]),
    );
    // φ = fround(h₀) mod 5 CI, in [0, 1,000) m at an interval of 200 m.
    const phases = instanceSlots(made.engine).map((slot) => {
      const key = drawn.get(slot)?.patch.key;
      const h0 = key === undefined ? Number.NaN : Math.fround(originHeightOf(key));
      return Math.fround(h0 - 1_000 * Math.floor(h0 / 1_000));
    });
    expect([phases.length > 1, phaseRecords(made.engine).map((record) => record[0])]).toEqual([
      true,
      phases,
    ]);
  });

  it("gives no height above a sea where the world has none", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, { ...WIRE, seaLevelM: null }));
    const above = phaseRecords(made.engine).map((record) => record[1]);
    expect([above.length > 0, above.filter((m) => m !== 0)]).toEqual([true, []]);
  });

  it("forms each patch's graticule place again once the step changes", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    const finer: TerrainWireFrameInput = {
      ...WIRE,
      graticule: { stepRad: (15 * Math.PI) / 180, thinPx: 2, heavyPx: 3, casingPx: 1 },
    };
    made.pass.drawFor(PRIMARY, frameOf(VIEW, finer));
    const records = phaseRecords(made.engine);
    expect([records.length > 1, records]).toEqual([true, expectedPhases(made, finer)]);
  });

  it("forms each patch's place with no graticule as a step of zero", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    const none: TerrainWireFrameInput = { ...WIRE, graticule: null };
    made.pass.drawFor(PRIMARY, frameOf(VIEW, none));
    const records = phaseRecords(made.engine);
    expect([records.length > 1, records]).toEqual([true, expectedPhases(made, none)]);
  });

  it("takes a rebaked patch's new origin and origin height", async () => {
    const made = await wirePass();
    streamed(made);
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    const patch = made.pass.lastStep?.drawSet.patches[0];
    if (patch === undefined) {
      throw new Error("the pass drew no patch");
    }
    const { key } = patch.patch;
    const o = originOf(key);
    const again = { originM: vec3(o.x * 1.01, o.y * 1.01, o.z * 1.01), originHeightM: 777.125 };
    made.lane().deliver(made.pass, key, again);
    made.pass.step({ views: [passView(VIEW)], grounded: [] });
    made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    const records = phaseRecords(made.engine);
    expect([records.length > 1, records]).toEqual([
      true,
      expectedPhases(made, WIRE, new Map([[patchKeyString(key), again]])),
    ]);
  });
});

describe("the wireframe's annunciation and mask (R10.T11)", () => {
  it("says no DETAIL LIMITED on the low setting once everything is resident, its τ and depth the high setting's", async () => {
    const made = await wirePass("low");
    streamed(made);
    expect(made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE, 2_000)).conditions).toEqual({
      streaming: false,
      detailLimited: false,
    });
  });

  it("still says DETAIL LIMITED on the low setting in the lit pass", async () => {
    const made = await passOver("low", LIT_BODY);
    const view = passView(VIEW, TERRAIN_SETTINGS.low.tauPx);
    streamed(made, view);
    expect(made.pass.drawFor(PRIMARY, frameOf(VIEW, undefined, 2_000)).conditions).toEqual({
      streaming: false,
      detailLimited: true,
    });
  });

  it("withholds the mask while every patch the view sees is missing", async () => {
    const made = await wirePass();
    stream(made, 0);
    expect([
      made.pass.maskReadyFor(PRIMARY),
      made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).maskReady,
    ]).toEqual([false, false]);
  });

  it("gives the mask once nothing the view sees is missing, with no phase table to wait for", async () => {
    const made = await wirePass();
    streamed(made);
    expect([
      made.pass.maskReadyFor(PRIMARY),
      made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE, 16)).maskReady,
    ]).toEqual([true, true]);
  });

  /**
   * A wireframe pass over a field of level 6, whose roots alone are of cell code 0: the roots baked
   * with `rootUnsurveyed` vertices outside the survey, every other patch whole, stepped once more
   * after the roots have landed.
   */
  async function rootsLanded(rootUnsurveyed: ReadonlyArray<number>): Promise<Made> {
    const made = await wirePass();
    made.lane().layers = wireLayers(6);
    made.pass.step({ views: [passView(VIEW)], grounded: [] });
    if (!made.lane().demand.every((r) => r.key.level === 0)) {
      throw new Error("the first demand is the roots'");
    }
    for (const request of made.lane().demand) {
      made.lane().layersOf.set(patchKeyString(request.key), wireLayers(6, rootUnsurveyed));
    }
    made.lane().bakeDemand(made.pass);
    made.pass.step({ views: [passView(VIEW)], grounded: [] });
    return made;
  }

  it("withholds the mask while it draws a root of cell code 0 with a vertex outside the survey, nothing missing", async () => {
    const made = await rootsLanded([0]);
    const frame = made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE));
    expect([
      made.pass.maskReadyFor(PRIMARY),
      frame.maskReady,
      frame.missing,
      frame.draw === null,
    ]).toEqual([false, false, false, false]);
  });

  it("gives the mask while it draws a root of cell code 0 with every vertex surveyed", async () => {
    const made = await rootsLanded([]);
    expect([
      made.pass.maskReadyFor(PRIMARY),
      made.pass.drawFor(PRIMARY, frameOf(VIEW, WIRE)).maskReady,
    ]).toEqual([true, true]);
  });
});
