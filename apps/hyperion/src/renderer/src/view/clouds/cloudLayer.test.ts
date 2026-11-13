import { describe, expect, it } from "vitest";

import type { SunState } from "../atmosphere/hillaire";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { BufferSpec, TextureSpec } from "../engine/memory";
import type { BufferHandle, TextureHandle } from "../engine/types";
import { CLOUD_OCTAVES_MAX, cloudFieldParams, cloudSeedOf, normalTail } from "./cloudDensity";
import {
  CLOUD_COVERAGE_HEIGHT,
  CLOUD_COVERAGE_WIDTH,
  CLOUD_LAYER_ROWS,
  CLOUD_LAYER_WGSL,
  CLOUD_SEASON_STEP_RAD,
  type CloudFrame,
  cloudLayerHeightM,
  CloudLayers,
  cloudLayerUniforms,
  cloudShellOf,
  layerCondensateOf,
  packCloudLayer,
  seasonCoverage,
  seasonOf,
} from "./cloudLayer";
import { CLOUD_NOISE_RANGE, CLOUD_NOISE_SIZE, CLOUD_NOISE_STD } from "./cloudNoise";
import { CLOUD_PROBE_WGSL } from "./cloudProbe";
import { type CloudCondensate, type CloudDeck, STAND_IN_CONDENSATES } from "./condensates";
import { rowSolidAngleShare } from "./coverage";

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

const DECK: CloudDeck = {
  condensate: "H2O",
  phase: "liquid",
  inCloudOpticalDepth550: 8,
  coverage: 0.6,
  baseAltitudeM: 1_000,
  topAltitudeM: 3_000,
  optics: null,
};

const FRAME: CloudFrame = {
  id: "0200080020000000.0103",
  clouds: { cloudFraction: 0.67, surfacePressurePa: 101_325, decks: [DECK] },
  condensate: CONDENSATE,
  figure: FIGURE,
  season: { subsolarLatitudeRad: 0.1 },
  time: { seconds: 1_000, nanos: 0 },
  rate: 1,
  octaves: 9,
};

/** The value of a WGSL `const NAME : type = value;` in `wgsl`, as a number. */
function wgslConst(wgsl: string, name: string): number {
  const found = new RegExp(String.raw`const ${name} : [a-z0-9]+ = ([-0-9.e]+)u?;`, "u").exec(wgsl);
  if (found?.[1] === undefined) {
    throw new Error(`no const ${name}`);
  }
  return Number(found[1]);
}

/** A recording stand-in for the engine's creation and write paths. */
class RecordingEngine {
  readonly buffers = new Map<BufferHandle, BufferSpec>();
  readonly textures = new Map<TextureHandle, TextureSpec>();
  /** Each texture write's texture and a copy of its bytes, in order. */
  readonly textureWrites: Array<{ readonly texture: TextureHandle; readonly bytes: Uint8Array }> =
    [];
  /** Each buffer write's bytes, in order. */
  readonly bufferWrites: Uint8Array[] = [];

  createBuffer(spec: BufferSpec): BufferHandle {
    const handle: BufferHandle = { kind: "buffer", name: spec.name, bytes: spec.bytes };
    this.buffers.set(handle, spec);
    return handle;
  }

  createTexture(spec: TextureSpec): TextureHandle {
    const handle: TextureHandle = { kind: "texture", name: spec.name };
    this.textures.set(handle, spec);
    return handle;
  }

  writeBuffer(_buffer: BufferHandle, _offsetBytes: number, data: ArrayBufferView): void {
    this.bufferWrites.push(new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice());
  }

  writeTexture(
    texture: TextureHandle,
    _origin: GPUOrigin3D,
    _size: GPUExtent3D,
    data: ArrayBufferView,
  ): void {
    this.textureWrites.push({
      texture,
      bytes: new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice(),
    });
  }

  releaseBuffer(buffer: BufferHandle): void {
    this.buffers.delete(buffer);
  }

  releaseTexture(texture: TextureHandle): void {
    this.textures.delete(texture);
  }

  /** How many times `texture` was written. */
  writesOf(texture: TextureHandle | undefined): number {
    return this.textureWrites.filter((write) => write.texture === texture).length;
  }
}

/** A layer whose noise tile is made. */
function readyLayers(engine: RecordingEngine): CloudLayers {
  const layers = new CloudLayers(engine, "test");
  layers.step(() => 0, Number.POSITIVE_INFINITY);
  return layers;
}

describe("the layer's WGSL", () => {
  it("holds the CPU's constants and rows", () => {
    for (const wgsl of [CLOUD_LAYER_WGSL, CLOUD_PROBE_WGSL]) {
      expect(wgslConst(wgsl, "CLOUD_LAYER_ROWS")).toBe(CLOUD_LAYER_ROWS);
      expect(wgslConst(wgsl, "CLOUD_OCTAVES_MAX")).toBe(CLOUD_OCTAVES_MAX);
      expect(wgslConst(wgsl, "CLOUD_NOISE_STD")).toBe(CLOUD_NOISE_STD);
      expect(wgslConst(wgsl, "CLOUD_NOISE_RANGE")).toBe(CLOUD_NOISE_RANGE);
    }
    // The offsets' rows, octave k's phase i at 10 + 2k + i, end at the last row.
    expect(wgslConst(CLOUD_LAYER_WGSL, "CLOUD_ROW_OFFSETS") + 2 * CLOUD_OCTAVES_MAX).toBe(
      CLOUD_LAYER_ROWS,
    );
  });
});

describe("packCloudLayer", () => {
  const params = cloudFieldParams({
    seed: cloudSeedOf(FRAME.id),
    condensate: CONDENSATE,
    radiusM: FIGURE.equatorialRadiusM,
    time: FRAME.time,
    rate: 1,
    windMps: 20,
    octaves: 12,
  });
  const shell = cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE));
  const rows = packCloudLayer(params, shell, CONDENSATE, FIGURE.equatorialRadiusM);
  const row = (i: number): number[] => Array.from(rows.subarray(4 * i, 4 * i + 4));

  it("lays the shell, optics, flow map, norm and rotation out as cloudField.wgsl reads them", () => {
    expect(rows.length).toBe(4 * CLOUD_LAYER_ROWS);
    expect(row(0)).toEqual(
      [shell.equatorialRadiusM, shell.polarRadiusM, shell.heightM, 8].map(Math.fround),
    );
    expect(row(1)).toEqual(
      [CONDENSATE.asymmetry, CONDENSATE.coAlbedo, 12, params.baseCellM].map(Math.fround),
    );
    expect(row(2)).toEqual([...params.advectionS, ...params.phaseWeights].map(Math.fround));
    const total = (1 - 0.49 ** 12) / (1 - 0.49);
    expect(row(3)[0]).toBeCloseTo(1 / Math.sqrt(total), 6);
    expect(row(3)[1]).toBe(Math.fround(FIGURE.equatorialRadiusM));
    expect(row(7).slice(0, 3)).toEqual(params.rotation.slice(0, 3).map(Math.fround));
    expect(row(10 + 2 * 11 + 1).slice(0, 3)).toEqual(
      Array.from(params.offsets.subarray(3 * 23, 3 * 24)).map(Math.fround),
    );
  });
});

describe("the layer's condensate and height", () => {
  it("draws no layer on a clear sky, an overcast or a body without a deck", () => {
    expect(layerCondensateOf({ ...FRAME.clouds, cloudFraction: 0 })).toBeNull();
    expect(layerCondensateOf({ ...FRAME.clouds, cloudFraction: 1 })).toBeNull();
    expect(layerCondensateOf({ ...FRAME.clouds, decks: [] })).toBeNull();
    expect(layerCondensateOf(FRAME.clouds)?.condensate).toBe("H2O");
  });

  it("stands a quarter of the deck's depth above its base", () => {
    expect(cloudLayerHeightM(CONDENSATE)).toBe(1_500);
  });
});

describe("seasonCoverage", () => {
  it("meets plan 14's fraction over the sphere through its thresholds", () => {
    const texels = seasonCoverage({ subsolarLatitudeRad: 0.2 }, 0.67);
    let mean = 0;
    for (let j = 0; j < CLOUD_COVERAGE_HEIGHT; j += 1) {
      mean +=
        rowSolidAngleShare(j, CLOUD_COVERAGE_HEIGHT) *
        normalTail(texels.texels[4 * j * CLOUD_COVERAGE_WIDTH] ?? 0);
    }
    // The f16 threshold's step, about 10⁻³ near 1, moves Q by under 4 × 10⁻⁴.
    expect(mean).toBeCloseTo(0.67, 3);
  });
});

describe("seasonOf", () => {
  it("is the brightest light's latitude in the body-fixed axes", () => {
    const season = seasonOf(IDENTITY_QUATERNION, { x: 1, y: 2, z: 3 }, { x: 1, y: 12, z: 13 });
    expect(season.subsolarLatitudeRad).toBeCloseTo(Math.PI / 4, 12);
  });
});

/** A source at zenith-ward component `z` in the body's y–z plane, in light slot `slot`. */
function sun(z: number, slot: number): SunState {
  return {
    directionBodyFixed: { x: 0, y: Math.sqrt(1 - z * z), z },
    illuminanceLx: [100_000, 90_000, 80_000],
    angularRadiusRad: 4.65e-3,
    curveSlot: slot,
  };
}

describe("cloudLayerUniforms", () => {
  const lit = (suns: SunState[]): Readonly<Record<string, Float32Array>> =>
    cloudLayerUniforms({
      camera: {
        positionM: { x: FIGURE.equatorialRadiusM + 5_000, y: 0, z: 0 },
        orientation: IDENTITY_QUATERNION,
        fovXRad: 1,
        viewport: { widthPx: 200, heightPx: 100 },
      },
      figure: FIGURE,
      shell: cloudShellOf(FIGURE, 1_500),
      airTopM: 100_000,
      suns,
      exposureScale: 1e-3,
      octaves: 9,
    });

  it("packs at most two sources, brightest first, each its direction, slot and exposed light", () => {
    const uniforms = lit([sun(0.6, 0), sun(0.8, 1), sun(0, 2)]);
    expect(uniforms["view"]?.[3]).toBe(2);
    expect(Array.from(uniforms["source0"] ?? [])).toEqual([0, 0.8, 0.6, 0].map(Math.fround));
    expect(Array.from(uniforms["source1"] ?? [])).toEqual([0, 0.6, 0.8, 1].map(Math.fround));
    expect(Array.from(uniforms["sourceLight0"] ?? [])).toEqual([100, 90, 80, 0].map(Math.fround));
  });

  it("carries the octaves the layer draws", () => {
    expect(lit([])["levers"]?.[0]).toBe(9);
  });

  it("forms the shell's quadric in f64, positive above the shell and negative below", () => {
    const shell = cloudShellOf(FIGURE, 1_500);
    const view = (heightM: number): Record<string, Float32Array> =>
      cloudLayerUniforms({
        camera: {
          positionM: { x: FIGURE.equatorialRadiusM + heightM, y: 0, z: 0 },
          orientation: IDENTITY_QUATERNION,
          fovXRad: 1,
          viewport: { widthPx: 200, heightPx: 100 },
        },
        figure: FIGURE,
        shell,
        airTopM: 100_000,
        suns: [],
        exposureScale: 1,
        octaves: 9,
      });
    const above = view(2_000)["shell"]?.[0] ?? 0;
    const below = view(1_000)["shell"]?.[0] ?? 0;
    expect(above).toBeCloseTo(2 * FIGURE.equatorialRadiusM * 500 + 500 * 500 + 2 * 1_500 * 500, -4);
    expect(below).toBeLessThan(0);
    expect(view(1_000)["view"]?.[3]).toBe(0);
  });
});

describe("CloudLayers", () => {
  it("draws nothing until its noise tile is made, spread over steps within their budget", () => {
    const engine = new RecordingEngine();
    const layers = new CloudLayers(engine, "test");
    expect(layers.frame(FRAME)).toBeNull();
    let clock = 0;
    layers.step(() => clock++, 3);
    expect(layers.ready).toBe(false);
    for (let i = 0; i < 2 * CLOUD_NOISE_SIZE && !layers.ready; i += 1) {
      layers.step(() => clock++, 3);
    }
    expect(layers.ready).toBe(true);
    const noise = [...engine.textures.values()];
    expect(noise.map((spec) => [spec.dimension, spec.format, spec.category])).toEqual([
      ["3d", "r8unorm", "clouds"],
    ]);
    layers.dispose();
  });

  it("makes a body's rows in the clouds' category", () => {
    const engine = new RecordingEngine();
    const gpu = readyLayers(engine).frame(FRAME);
    const spec = gpu === null ? undefined : engine.buffers.get(gpu.layer);
    expect(spec).toMatchObject({ bytes: 16 * CLOUD_LAYER_ROWS, category: "clouds" });
  });

  it("makes a body's coverage texture in the clouds' category", () => {
    const engine = new RecordingEngine();
    const gpu = readyLayers(engine).frame(FRAME);
    const spec = gpu === null ? undefined : engine.textures.get(gpu.coverage);
    expect(spec).toMatchObject({
      size: { width: CLOUD_COVERAGE_WIDTH, height: CLOUD_COVERAGE_HEIGHT },
      format: "rgba16float",
      category: "clouds",
    });
  });

  it("writes a body's coverage again only when its season moves a step or its fraction changes", () => {
    const engine = new RecordingEngine();
    const layers = readyLayers(engine);
    const coverage = layers.frame(FRAME)?.coverage;
    expect(engine.writesOf(coverage)).toBe(1);
    layers.frame({ ...FRAME, time: { seconds: 2_000, nanos: 0 } });
    expect(engine.writesOf(coverage)).toBe(1);
    layers.frame({
      ...FRAME,
      season: { subsolarLatitudeRad: FRAME.season.subsolarLatitudeRad + CLOUD_SEASON_STEP_RAD },
    });
    expect(engine.writesOf(coverage)).toBe(2);
    layers.frame({ ...FRAME, clouds: { ...FRAME.clouds, cloudFraction: 0.5 } });
    expect(engine.writesOf(coverage)).toBe(3);
  });

  it("writes a body's rows at every frame", () => {
    const engine = new RecordingEngine();
    const layers = readyLayers(engine);
    layers.frame(FRAME);
    layers.frame({ ...FRAME, time: { seconds: 2_000, nanos: 0 } });
    expect(engine.bufferWrites).toHaveLength(2);
  });

  it("gives a client joining late the same rows and coverage as one drawing from earlier, bit for bit", () => {
    const early = new RecordingEngine();
    const earlyLayers = readyLayers(early);
    earlyLayers.frame({
      ...FRAME,
      time: { seconds: 0, nanos: 0 },
      season: { subsolarLatitudeRad: 0 },
    });
    earlyLayers.frame(FRAME);
    const late = new RecordingEngine();
    readyLayers(late).frame(FRAME);
    expect(late.bufferWrites.at(-1)).toEqual(early.bufferWrites.at(-1));
    expect(late.textureWrites.at(-1)?.bytes).toEqual(early.textureWrites.at(-1)?.bytes);
  });

  it("lets go of a body no frame draws, and of everything when disposed", () => {
    const engine = new RecordingEngine();
    const layers = readyLayers(engine);
    layers.frame(FRAME);
    layers.frame({ ...FRAME, id: "0200080020000000.0104" });
    expect(engine.buffers.size).toBe(2);
    layers.keepOnly(new Set([FRAME.id]));
    expect(engine.buffers.size).toBe(1);
    expect(engine.textures.size).toBe(2);
    layers.dispose();
    expect(engine.buffers.size).toBe(0);
    expect(engine.textures.size).toBe(0);
  });
});
