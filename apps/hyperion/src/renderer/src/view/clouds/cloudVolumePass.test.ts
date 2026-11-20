import { describe, expect, it } from "vitest";

import { steppingClock } from "../../test/cloudFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { createAirlessTables } from "../atmosphere/surfaceLighting";
import { lookAlong } from "../camera/quaternion";
import type { MaterialHandle, MeshHandle, RenderTarget, ViewSize } from "../engine/types";
import { CLOUD_LAYER_PASS, type CloudLayerView, cloudShellOf } from "./cloudLayer";
import { CLOUD_LEAST_MU, CLOUD_SKY_MU } from "./cloudShadow";
import { octaveCorrectionOf } from "./cloudVolume";
import { BASE_NOISE_SIZE, CURL_NOISE_SIZE, DETAIL_NOISE_SIZE } from "./cloudShapeNoise";
import type { ShapeNoiseRequest } from "./cloudShapeNoise";
import {
  CLOUD_MAP_PASS,
  CLOUD_RESOLVE_PASS,
  CLOUD_SHAPE_ROWS,
  CLOUD_VOLUME_PASS,
  CLOUD_VOLUME_WGSL,
  CloudVolumes,
  compositeUniforms,
  heightAboveDeckM,
  OctaveCorrectionBuilder,
  packCloudShape,
  reprojectedUv,
  TEMPORAL_WEIGHT,
  VOLUME_FLAT_FROM_M,
  VOLUME_LEVERS,
  VOLUME_REACH_M,
  volumeCameraOf,
  volumeFlatteningAt,
  volumeJitter,
  volumeStepOf,
  volumeTargetSize,
  volumeWanted,
  windAtM,
} from "./cloudVolumePass";
import { cloudShapeFrame, cloudShapeKindOf, normalisationTable } from "./cloudShape";
import { type CloudCondensate, STAND_IN_CONDENSATES } from "./condensates";

const FIGURE = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.3 };

const CONDENSATE: CloudCondensate = {
  condensate: "H2O",
  phase: "liquid",
  asymmetry: STAND_IN_CONDENSATES.liquidWater.asymmetry,
  coAlbedo: STAND_IN_CONDENSATES.liquidWater.coAlbedo,
  inCloudOpticalDepth550: 8,
  baseAltitudeM: 1_000,
  topAltitudeM: 3_000,
  opticsSource: "stand-in",
  opticalDepthSource: "stand-in",
};

const SIZE: ViewSize = { widthPx: 65, heightPx: 36 };

/** The layer's view of an Earth from `heightM` over its equator, looking along `direction`. */
function viewFrom(heightM: number, direction = { x: -1, y: 0, z: 0 }): CloudLayerView {
  return {
    camera: {
      positionM: { x: FIGURE.equatorialRadiusM + heightM, y: 0, z: 0 },
      orientation: lookAlong(direction, { x: 0, y: 0, z: 1 }),
      fovXRad: Math.PI / 3,
      viewport: SIZE,
    },
    figure: FIGURE,
    shell: cloudShellOf(FIGURE, 1_500),
    airTopM: 100_000,
    suns: [],
    exposureScale: 1e-4,
    octaves: 12,
  };
}

/** A shape-noise worker that answers at once with blank noises of the right sizes. */
class InstantNoiseWorker {
  readonly #listeners: Array<(event: MessageEvent<unknown>) => void> = [];

  addEventListener(type: "message", listener: (event: MessageEvent<unknown>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  addEventListener(type: string, listener: (event: MessageEvent<unknown>) => void): void {
    if (type === "message") {
      this.#listeners.push(listener);
    }
  }

  postMessage(message: ShapeNoiseRequest): void {
    const noises = {
      base: new Uint8Array(BASE_NOISE_SIZE ** 3),
      detail: new Uint8Array(DETAIL_NOISE_SIZE ** 3),
      curl: new Uint8Array(CURL_NOISE_SIZE ** 2),
    };
    for (const listener of this.#listeners) {
      listener(new MessageEvent("message", { data: { kind: "made", seed: message.seed, noises } }));
    }
  }

  terminate(): void {}
}

describe("the volume's correction table, made over frames (R11.T7.c)", () => {
  it("is octaveCorrectionOf's, bit for bit, at its last row, and none before", () => {
    const builder = new OctaveCorrectionBuilder(CONDENSATE);
    const made: Array<Float32Array | null> = [];
    for (let row = 0; row < 16; row += 1) {
      made.push(builder.step());
    }
    expect(made.slice(0, 15).every((step) => step === null)).toBe(true);
    expect(made[15]).toEqual(octaveCorrectionOf(CONDENSATE).factors);
  });
});

describe("the volume's targets and jitter (R11.T7.c)", () => {
  it("marches at half the view's width and height, rounded up", () => {
    expect(volumeTargetSize(SIZE)).toEqual({ widthPx: 33, heightPx: 18 });
    expect(volumeTargetSize({ widthPx: 1, heightPx: 1 })).toEqual({ widthPx: 1, heightPx: 1 });
  });

  it("spreads the frames' jitter over [0, 1), no gap of 16 frames' wider than a seventh", () => {
    const jitters = Array.from({ length: 16 }, (_, i) => volumeJitter(i)).toSorted((a, b) => a - b);
    expect(jitters.every((j) => j >= 0 && j < 1)).toBe(true);
    const gaps = jitters.map((j, i) => (jitters[i + 1] ?? 1 + (jitters[0] ?? 0)) - j);
    expect(Math.max(...gaps)).toBeLessThan(1 / 7);
  });
});

/** The place where pixel (u, v)'s ray stands `distanceM` from `camera`. */
function placeAt(view: CloudLayerView, u: number, v: number, distanceM: number) {
  const camera = volumeCameraOf(view);
  const x = 2 * u - 1;
  const y = 1 - 2 * v;
  const ray = {
    x: camera.forward.x + x * camera.right.x + y * camera.up.x,
    y: camera.forward.y + x * camera.right.y + y * camera.up.y,
    z: camera.forward.z + x * camera.right.z + y * camera.up.z,
  };
  const length = Math.hypot(ray.x, ray.y, ray.z);
  return {
    x: camera.positionM.x + (distanceM * ray.x) / length,
    y: camera.positionM.y + (distanceM * ray.y) / length,
    z: camera.positionM.z + (distanceM * ray.z) / length,
  };
}

describe("reprojectedUv (R11.T7.c)", () => {
  it("finds a place where the camera that saw it saw it", () => {
    const view = viewFrom(400_000);
    const uv = reprojectedUv(placeAt(view, 0.3, 0.7, 50_000), volumeCameraOf(view));
    expect(uv?.u).toBeCloseTo(0.3, 9);
    expect(uv?.v).toBeCloseTo(0.7, 9);
  });

  it("moves a place a camera sees across its view as the camera moves past it", () => {
    const view = viewFrom(400_000);
    const place = placeAt(view, 0.5, 0.5, 50_000);
    const moved = volumeCameraOf(view);
    const uv = reprojectedUv(place, {
      ...moved,
      positionM: { ...moved.positionM, y: moved.positionM.y + 5_000 },
    });
    expect(uv?.v).toBeCloseTo(0.5, 9);
    expect(Math.abs((uv?.u ?? 0.5) - 0.5)).toBeGreaterThan(0.05);
  });

  it("finds none behind the camera", () => {
    const view = viewFrom(400_000);
    const camera = volumeCameraOf(view);
    const behind = {
      x: camera.positionM.x - 1_000 * camera.forward.x,
      y: camera.positionM.y - 1_000 * camera.forward.y,
      z: camera.positionM.z - 1_000 * camera.forward.z,
    };
    expect(reprojectedUv(behind, camera)).toBeNull();
  });
});

describe("compositeUniforms (R11.T7.c)", () => {
  it.each([
    ["above the deck", 10_000, [true, true]],
    ["within it", 2_000, [false, true]],
    ["beneath it", 100, [false, false]],
  ] as const)("forms the deck's quadric constants in f64 from %s", (_, heightM, signs) => {
    const deck = compositeUniforms(viewFrom(heightM), CONDENSATE)["deck"];
    expect([(deck?.[1] ?? 0) > 0, (deck?.[2] ?? 0) > 0]).toEqual(signs);
  });
});

describe("the volume's march WGSL (R11.T7.c)", () => {
  it("holds the layer's least cosine and its sky's cosine", () => {
    expect(CLOUD_VOLUME_WGSL).toContain(
      `const CLOUD_MARCH_LEAST_MU : f32 = ${String(CLOUD_LEAST_MU)};`,
    );
    expect(CLOUD_VOLUME_WGSL).toContain(
      `const CLOUD_MARCH_SKY_COSINE : f32 = ${String(CLOUD_SKY_MU)};`,
    );
  });
});

describe("CloudVolumes (R11.T7.c)", () => {
  const MATERIALS = {
    march: { kind: "material", name: "cloud volume" },
    resolve: { kind: "material", name: "cloud volume resolve" },
    composite: { kind: "material", name: "cloud volume composite" },
    map: { kind: "material", name: "cloud map" },
    shadow: { kind: "material", name: "cloud shadow map" },
    triangle: { kind: "mesh", name: "triangle" },
  } as const satisfies {
    readonly march: MaterialHandle;
    readonly resolve: MaterialHandle;
    readonly composite: MaterialHandle;
    readonly map: MaterialHandle;
    readonly shadow: MaterialHandle;
    readonly triangle: MeshHandle;
  };
  const PASS = { viewRotation: new Float32Array(16), projection: new Float32Array(16) };

  /** A volume on the counting engine, its draw's inputs, and the air target it composites into. */
  async function volumeOn() {
    const engine = await countingRenderEngine();
    const volumes = new CloudVolumes(engine, "view", new InstantNoiseWorker());
    const texture = (name: string) =>
      engine.createTexture({
        name,
        size: { width: 1, height: 1 },
        dimension: "2d",
        format: "rgba16float",
        mips: 1,
        usage: 0,
        category: "clouds",
      });
    const gpu = {
      layer: engine.createBuffer({ name: "rows", bytes: 544, usage: 0, category: "clouds" }),
      coverage: texture("coverage"),
      noise: texture("noise"),
      shadowMap: texture("no shadow"),
      shadowRows: engine.createBuffer({
        name: "no shadow rows",
        bytes: 144,
        usage: 0,
        category: "clouds",
      }),
    };
    const air: RenderTarget = engine.createRenderTarget({
      name: "air",
      size: SIZE,
      format: "rgba16float",
      mips: 1,
      depth: false,
      category: "render-targets",
    });
    const draw = (id: string, correction = texture("correction")) => {
      volumes.noisesReady(id, () => 0);
      return {
        id,
        gpu,
        condensate: CONDENSATE,
        view: viewFrom(4_000),
        lighting: createAirlessTables(engine, "no air"),
        sceneDepth: texture("depth"),
        correction,
        timeS: 1_000,
        rate: 1,
        levers: VOLUME_LEVERS.full,
      };
    };
    return { engine, volumes, air, draw };
  }

  it("makes a condensate's correction within its budget a frame, a row at least", async () => {
    const { volumes } = await volumeOn();
    const clock = steppingClock();
    const made = Array.from({ length: 16 }, () => volumes.correction(CONDENSATE, clock));
    expect(made.slice(0, 15).every((texture) => texture === null)).toBe(true);
    expect(made[15]).not.toBeNull();
    // Made once: the next frames take it.
    expect(volumes.correction(CONDENSATE, clock)).toBe(made[15]);
  });

  it("maps, marches and resolves into its own targets, then composites over the air, loading it", async () => {
    const { engine, volumes, air, draw } = await volumeOn();
    volumes.draw(draw("A"), MATERIALS, air, SIZE, PASS, null);
    expect(engine.targetFrames.map((f) => f.label)).toEqual([
      CLOUD_MAP_PASS,
      CLOUD_VOLUME_PASS,
      CLOUD_RESOLVE_PASS,
      CLOUD_LAYER_PASS,
    ]);
    const [map, march, resolve, composite] = engine.targetFrames;
    expect(march?.draws[0]?.textures["cloudMap"]).toBeDefined();
    expect(map?.draws[0]?.uniforms["mapOrigins4"]?.[2]).toBe(256);
    expect(march?.draws[0]?.uniforms["view"]?.slice(0, 2)).toEqual(new Float32Array([33, 18]));
    expect(composite?.colourLoad).toBe("load");
    expect(resolve?.draws[0]?.textures["cloudMarch"]).toBeDefined();
  });

  it("resolves from the march alone at its first frame, from its history after, and alone again after a cut or for another body", async () => {
    const { engine, volumes, air, draw } = await volumeOn();
    const held = (): number | undefined =>
      engine.targetFrames.findLast((f) => f.label === CLOUD_RESOLVE_PASS)?.draws[0]?.uniforms[
        "camera"
      ]?.[3];
    const frames: Array<number | undefined> = [];
    for (const id of ["A", "A"]) {
      volumes.draw(draw(id), MATERIALS, air, SIZE, PASS, null);
      frames.push(held());
    }
    volumes.cut();
    volumes.draw(draw("A"), MATERIALS, air, SIZE, PASS, null);
    frames.push(held());
    volumes.draw(draw("B"), MATERIALS, air, SIZE, PASS, null);
    frames.push(held());
    expect(frames).toEqual([0, 1, 0, 0]);
    const weight = engine.targetFrames.findLast((f) => f.label === CLOUD_RESOLVE_PASS)?.draws[0]
      ?.uniforms["shell"]?.[3];
    expect(weight).toBeCloseTo(TEMPORAL_WEIGHT, 7);
  });

  it("reads the history the last frame resolved into, and resolves into the other", async () => {
    const { engine, volumes, air, draw } = await volumeOn();
    volumes.draw(draw("A"), MATERIALS, air, SIZE, PASS, null);
    volumes.draw(draw("A"), MATERIALS, air, SIZE, PASS, null);
    const composites = engine.targetFrames.filter((f) => f.label === CLOUD_LAYER_PASS);
    const resolves = engine.targetFrames.filter((f) => f.label === CLOUD_RESOLVE_PASS);
    const first = composites[0]?.draws[0]?.textures["cloudResolved"];
    expect(resolves[1]?.draws[0]?.textures["cloudHistory"]).toBe(first);
    expect(composites[1]?.draws[0]?.textures["cloudResolved"]).not.toBe(first);
  });

  it("makes four targets, a correction and the world's noises, and lets go of them all", async () => {
    const { engine, volumes, air, draw } = await volumeOn();
    const before = engine.live.textures;
    // The draw's inputs: its correction, depth, blue noise and airless tables, and the world's
    // base, detail and curl noises, made as it asks for them.
    const inputs = draw("A");
    const asked = engine.live.textures;
    volumes.correction(CONDENSATE, () => 0, Number.POSITIVE_INFINITY);
    volumes.draw(inputs, MATERIALS, air, SIZE, PASS, null);
    // The correction; the march's target, two histories and the map.
    expect(engine.live.textures).toBe(asked + 5);
    volumes.dispose();
    // The noises and the correction let go; the fake's targets keep their textures while it lives
    // (`CountingRenderEngine.live`).
    expect(engine.live.textures).toBe(asked + 4 - 3);
    expect(asked - before).toBeGreaterThanOrEqual(3);
  });

  it("refuses to draw a world whose noises are not made", async () => {
    const { volumes, air, draw } = await volumeOn();
    const inputs = draw("A");
    expect(() => {
      volumes.draw({ ...inputs, id: "B" }, MATERIALS, air, SIZE, PASS, null);
    }).toThrow(/noises/);
  });

  it("draws with the levers the floor's step gives it", async () => {
    const { engine, volumes, air, draw } = await volumeOn();
    volumes.draw({ ...draw("A"), levers: VOLUME_LEVERS.stepped }, MATERIALS, air, SIZE, PASS, null);
    const march = engine.targetFrames.findLast((f) => f.label === CLOUD_VOLUME_PASS);
    const levers = march?.draws[0]?.uniforms["levers"];
    expect([levers?.[0], levers?.[1], levers?.[3]]).toEqual([40, 4, 0]);
  });

  it("makes its targets again at a new size, its history dropped", async () => {
    const { engine, volumes, air, draw } = await volumeOn();
    volumes.draw(draw("A"), MATERIALS, air, SIZE, PASS, null);
    volumes.draw(draw("A"), MATERIALS, air, { widthPx: 130, heightPx: 72 }, PASS, null);
    const march = engine.targetFrames.findLast((f) => f.label === CLOUD_VOLUME_PASS);
    const resolve = engine.targetFrames.findLast((f) => f.label === CLOUD_RESOLVE_PASS);
    expect(march?.draws[0]?.uniforms["view"]?.slice(0, 2)).toEqual(new Float32Array([65, 36]));
    expect(resolve?.draws[0]?.uniforms["camera"]?.[3]).toBe(0);
  });
});

describe("when the volume draws (R11.T7.c; the owner's requirement, 2026-11-19)", () => {
  it("keeps the volume within the deck's reach whatever the floor's step, on high alone", () => {
    for (const height of [2, 500, 2_000, 10_000, VOLUME_REACH_M - 1]) {
      expect(volumeWanted("high", height)).toBe(true);
      expect(volumeWanted("low", height)).toBe(false);
    }
    expect(volumeWanted("high", VOLUME_REACH_M)).toBe(false);
  });

  it("degrades the volume at each step of the floor and never sheds it", () => {
    expect(volumeStepOf("high", undefined, undefined)).toBe("full");
    expect(volumeStepOf("high", "high", false)).toBe("full");
    expect(volumeStepOf("high", "low", false)).toBe("stepped");
    expect(volumeStepOf("high", "low", true)).toBe("lowest");
    const { full, stepped, lowest } = VOLUME_LEVERS;
    expect(full.viewSteps > stepped.viewSteps && stepped.viewSteps > lowest.viewSteps).toBe(true);
    expect(full.lightSteps > stepped.lightSteps && stepped.lightSteps > lowest.lightSteps).toBe(
      true,
    );
    expect([full.detail, stepped.detail, lowest.detail]).toEqual([true, false, false]);
  });

  it("flattens towards the layer's slab from 100 km above the tops to its reach, where the layer takes over", () => {
    expect(volumeFlatteningAt(0)).toBe(0);
    expect(volumeFlatteningAt(VOLUME_FLAT_FROM_M)).toBe(0);
    expect(volumeFlatteningAt((VOLUME_FLAT_FROM_M + VOLUME_REACH_M) / 2)).toBeCloseTo(0.5, 9);
    expect(volumeFlatteningAt(VOLUME_REACH_M)).toBe(1);
  });

  it("measures the camera's height above the deck's top on the datum", () => {
    const camera = { x: FIGURE.equatorialRadiusM + 3_500, y: 0, z: 0 };
    expect(heightAboveDeckM(camera, FIGURE, CONDENSATE)).toBeCloseTo(500, 6);
  });
});

describe("the volume's shape rows (R11.T7.c)", () => {
  it("packs the noises' tiles and drifts, the kind and κ's normalisation", () => {
    const frame = cloudShapeFrame(2_000, { x: 10, y: 0, z: 0 }, 100, 1);
    const kind = cloudShapeKindOf(CONDENSATE);
    const rows = packCloudShape(frame, kind);
    expect(rows.length).toBe(4 * CLOUD_SHAPE_ROWS);
    expect(rows[0]).toBeCloseTo(1 / 8_000, 9);
    expect(rows[1]).toBeCloseTo(1 / 1_000, 9);
    expect(rows[3]).toBeCloseTo(2 / 3, 6);
    // 10 m s⁻¹ eastward for 100 s against an 8 km tile: an eighth of a tile upwind.
    expect(rows[4]).toBeCloseTo(1 - 1_000 / 8_000, 6);
    expect(rows[14]).toBe(1);
    expect(rows.slice(16, 16 + 81)).toEqual(normalisationTable(kind.profileExponent));
  });

  it("takes the coverage's wind at the camera along its east", () => {
    const coverage = { widthCells: 1, heightCells: 1, texels: new Float32Array([0, 1, 12, 0]) };
    const wind = windAtM(coverage, { x: FIGURE.equatorialRadiusM, y: 0, z: 0 });
    expect(wind.y).toBeCloseTo(12, 9);
    expect(Math.abs(wind.x) + Math.abs(wind.z)).toBeLessThan(1e-9);
  });
});
