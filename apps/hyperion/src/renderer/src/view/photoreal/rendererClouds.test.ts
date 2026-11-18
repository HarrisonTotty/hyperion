import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { ATMOSPHERE_COMPOSITE_PASS } from "../atmosphere/hillaire";
import { R05_EARTH_REFERENCE } from "../atmosphere/earth";
import { AtmosphereTables } from "../atmosphere/tables";
import type { BodyFramePlan, LitBodyInput } from "../bodies/draw";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { CLOUD_LAYER_MATERIAL, CLOUD_LAYER_PASS, CLOUD_TEXTURES } from "../clouds/cloudLayer";
import { CLOUD_RESOLVE_PASS, CLOUD_VOLUME_PASS } from "../clouds/cloudVolumePass";
import type { BodyClouds } from "../clouds/condensates";
import type { DrawItem, FrameSubmission, RenderView } from "../engine/types";
import { AU_M } from "../scenes/kept";
import {
  type FrameAtmosphere,
  type FrameClouds,
  type FrameTerrain,
  type PhotorealFrame,
  PhotorealRenderer,
  withCloudsOnGround,
} from "./renderer";

const VIEWPORT = { widthPx: 64, heightPx: 36 };

/** The tables' ground, m. */
const EARTH_RADIUS_M = 6.371e6;

/** A canvas view that records the frames submitted to it. */
class RecordingView implements RenderView {
  readonly name = "test view";
  readonly frames: FrameSubmission[] = [];
  resize(): void {}
  render(frame: FrameSubmission): void {
    this.frames.push(frame);
  }
  readBack(): Promise<Uint8Array> {
    return Promise.resolve(new Uint8Array(0));
  }
  dispose(): void {}
}

/** An Earth-like body's clouds: a liquid water deck from 1 km to 3 km under 67% cover. */
const EARTH_CLOUDS: BodyClouds = {
  cloudFraction: 0.67,
  surfacePressurePa: 101_325,
  decks: [
    {
      condensate: "H2O",
      phase: "liquid",
      inCloudOpticalDepth550: 8,
      coverage: 0.67,
      baseAltitudeM: 1_000,
      topAltitudeM: 3_000,
      optics: null,
    },
  ],
};

function bodyAt(id: string, centreM: ReturnType<typeof vec3>): LitBodyInput {
  const lit = aLitBody();
  return { id, centreM, figure: lit.figure, photometry: lit.photometry, lighting: undefined };
}

/** A body's atmosphere in the frame over `tables`, its body-fixed axes the galactic ones. */
function airOf(
  body: LitBodyInput,
  tables: AtmosphereTables,
  clouds: BodyClouds | null,
): FrameAtmosphere {
  return {
    id: body.id,
    tables,
    camera: {
      positionM: vec3(-body.centreM.x, -body.centreM.y, -body.centreM.z),
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
      viewport: VIEWPORT,
    },
    toBodyFixed: IDENTITY_QUATERNION,
    clouds,
  };
}

/** A frame of one planet `body` with its air over `tables` and its `clouds`. */
function frameOf(
  body: LitBodyInput,
  tables: AtmosphereTables,
  clouds: BodyClouds | null,
): PhotorealFrame {
  return {
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    role: "eye",
    setting: "high",
    exposureScale: 1e-4,
    time: { seconds: 1_000, nanos: 0 },
    timeRate: 1,
    sky: [],
    starSprites: [],
    hostDraws: new Map(),
    glareSources: [],
    lights: [{ disc: aHostDisc(), centreM: vec3(AU_M, 0, -1e8) }],
    bodies: [body],
    depthWriters: [],
    previousRegimes: new Map(),
    overlay: null,
    meter: "average",
    atmospheres: [airOf(body, tables, clouds)],
  };
}

/**
 * A clock that moves a second each time it is read, so that a frame makes one slice of the noise
 * tile, whatever the machine.
 */
function steppingClock(): () => number {
  let nowMs = 0;
  return () => {
    nowMs += 1_000;
    return nowMs;
  };
}

/** A prepared renderer on the counting engine, its clouds' noise tile made a slice a frame. */
async function preparedRenderer(): Promise<{
  readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
  readonly view: RecordingView;
  readonly renderer: PhotorealRenderer;
}> {
  const engine = await countingRenderEngine();
  const view = new RecordingView();
  const renderer = new PhotorealRenderer(engine, "test view", { nowMs: steppingClock() });
  await renderer.prepare(VIEWPORT, "high", "eye", {
    orientation: IDENTITY_QUATERNION,
    fovXRad: Math.PI / 3,
  });
  return { engine, view, renderer };
}

describe("the photorealistic renderer's clouds (R11.T7.b)", () => {
  it("compiles the cloud layer with the view's pipelines", async () => {
    const { engine, renderer } = await preparedRenderer();
    expect(engine.compiles.map((compile) => compile.name)).toContain(CLOUD_LAYER_MATERIAL.name);
    renderer.dispose();
  });

  it("releases the cloud layer with the view", async () => {
    const { engine, renderer } = await preparedRenderer();
    const released = vi.spyOn(engine, "releaseMaterial");
    renderer.dispose();
    expect(released.mock.calls.map(([material]) => material.name)).toContain(
      CLOUD_LAYER_MATERIAL.name,
    );
  });

  it("draws a body's clouds after its composite, over the same area, once their noise tile is made", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, frameOf(planet, tables, EARTH_CLOUDS));
    // Before the tile is made the frame draws none.
    expect(engine.targetFrames.map((f) => f.label)).not.toContain(CLOUD_LAYER_PASS);
    renderer.prepareClouds();
    const from = engine.targetFrames.length;
    renderer.render(view, frameOf(planet, tables, EARTH_CLOUDS));
    const frames = engine.targetFrames.slice(from);
    const labels = frames.map((f) => f.label);
    const at = labels.indexOf(CLOUD_LAYER_PASS);
    expect(labels[at - 1]).toBe(ATMOSPHERE_COMPOSITE_PASS);
    const composite = frames[at - 1];
    const clouds = frames[at];
    expect(clouds?.colourLoad).toBe("load");
    expect(clouds?.scissor).toEqual(composite?.scissor);
    const draw = clouds?.draws[0];
    expect(draw?.material.name).toBe(CLOUD_LAYER_MATERIAL.name);
    expect(Object.keys(draw?.textures ?? {})).toEqual(
      expect.arrayContaining(["sceneDepth", CLOUD_TEXTURES.coverage, CLOUD_TEXTURES.noise]),
    );
    expect(draw?.storageBuffers?.[CLOUD_TEXTURES.layer]).toBeDefined();
    // The high setting's twelve octaves at most.
    expect(draw?.uniforms["levers"]?.[0]).toBe(12);
    renderer.dispose();
    tables.dispose();
  });

  it.each([
    ["a clear sky", { ...EARTH_CLOUDS, cloudFraction: 0 }],
    ["an overcast", { ...EARTH_CLOUDS, cloudFraction: 1 }],
    ["no deck", { ...EARTH_CLOUDS, decks: [] }],
  ])("draws no clouds under %s", async (_, clouds) => {
    const { engine, view, renderer } = await preparedRenderer();
    renderer.prepareClouds();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, frameOf(planet, tables, clouds));
    renderer.render(view, frameOf(planet, tables, null));
    expect(engine.targetFrames.map((f) => f.label)).not.toContain(CLOUD_LAYER_PASS);
    renderer.dispose();
    tables.dispose();
  });

  it("lets a body's clouds go when the frame no longer draws them", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    renderer.prepareClouds();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const before = engine.live.textures;
    renderer.render(view, frameOf(planet, tables, EARTH_CLOUDS));
    const drawing = engine.live.textures;
    renderer.render(view, frameOf(planet, tables, null));
    // Its coverage texture let go; the noise tile, made once a view, stays.
    expect(engine.live.textures).toBe(drawing - 1);
    expect(drawing).toBeGreaterThan(before);
    renderer.dispose();
    tables.dispose();
  });

  it("lights a handed-over body's terrain beneath its clouds through its record (cloudShadows)", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    renderer.prepareClouds();
    const planet = { ...bodyAt("0200080020000000.0300", vec3(0, 0, -1e8)), handoverReliefM: 1e9 };
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const plans: BodyFramePlan[] = [];
    const lit: Array<ReadonlyArray<LitBodyInput>> = [];
    const terrain: FrameTerrain = {
      surveyCoverage: () => null,
      surveyedWhole: () => false,
      draws: (
        plan: BodyFramePlan,
        bodies: ReadonlyArray<LitBodyInput>,
      ): ReadonlyArray<DrawItem> => {
        plans.push(plan);
        lit.push(bodies);
        return [];
      },
      symbologyDepth: () => [],
      sceneDepth: () => false,
      lateDraws: () => [],
      visibility: () => null,
      opaqueDraws: () => [],
    };
    renderer.render(view, { ...frameOf(planet, tables, EARTH_CLOUDS), terrain });
    const record = plans.at(-1)?.discs.find((each) => each.body === planet.id);
    const body = lit.at(-1)?.find((each) => each.id === planet.id);
    expect(record?.clouds).toBeDefined();
    expect(body?.clouds).toBe(record?.clouds);
    renderer.dispose();
    tables.dispose();
  });
});

describe("the photorealistic renderer's cloud volume (R11.T7.c)", () => {
  /** The labels of the last of `frames` renders of `planet`'s clouds on `setting`. */
  async function lastFrameLabels(
    setting: "high" | "low",
    frames: number,
    atmosphereSetting?: "low",
  ): Promise<ReadonlyArray<string>> {
    const { engine, view, renderer } = await preparedRenderer();
    renderer.prepareClouds();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const frame = {
      ...frameOf(planet, tables, EARTH_CLOUDS),
      setting,
      ...(atmosphereSetting === undefined ? {} : { atmosphereSetting }),
    };
    let from = 0;
    for (let i = 0; i < frames; i += 1) {
      from = engine.targetFrames.length;
      renderer.render(view, frame);
    }
    const labels = engine.targetFrames.slice(from).map((f) => f.label);
    renderer.dispose();
    tables.dispose();
    return labels;
  }

  it("draws the nearest body's clouds as the volume on high once its correction is made, the layer until then", async () => {
    const early = await lastFrameLabels("high", 1);
    const made = await lastFrameLabels("high", 16);
    expect([early.includes(CLOUD_VOLUME_PASS), made.includes(CLOUD_VOLUME_PASS)]).toEqual([
      false,
      true,
    ]);
    const at = made.indexOf(CLOUD_VOLUME_PASS);
    expect(made.slice(at - 1, at + 3)).toEqual([
      ATMOSPHERE_COMPOSITE_PASS,
      CLOUD_VOLUME_PASS,
      CLOUD_RESOLVE_PASS,
      CLOUD_LAYER_PASS,
    ]);
  });

  it("draws the layer on low, and on high while the frame-rate floor steps the air down", async () => {
    const low = await lastFrameLabels("low", 16);
    const stepped = await lastFrameLabels("high", 16, "low");
    expect([low.includes(CLOUD_VOLUME_PASS), stepped.includes(CLOUD_VOLUME_PASS)]).toEqual([
      false,
      false,
    ]);
    expect([low.includes(CLOUD_LAYER_PASS), stepped.includes(CLOUD_LAYER_PASS)]).toEqual([
      true,
      true,
    ]);
  });
});

describe("the cloud volume's materials and history in the renderer (R11.T7.c)", () => {
  const VOLUME_NAMES = ["cloud volume", "cloud volume resolve", "cloud volume composite"];

  it("releases the volume's materials with the view", async () => {
    const { engine, renderer } = await preparedRenderer();
    const released = vi.spyOn(engine, "releaseMaterial");
    renderer.dispose();
    expect(released.mock.calls.map(([material]) => material.name)).toEqual(
      expect.arrayContaining(VOLUME_NAMES),
    );
  });

  it("draws the layer on high where a volume material is refused, its others released", async () => {
    const engine = await countingRenderEngine();
    engine.failMaterials((name) => name === "cloud volume resolve");
    const warned = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const released = vi.spyOn(engine, "releaseMaterial");
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view", { nowMs: steppingClock() });
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    expect(released.mock.calls.map(([material]) => material.name)).toEqual(
      expect.arrayContaining(["cloud volume", "cloud volume composite"]),
    );
    expect(warned).toHaveBeenCalled();
    renderer.prepareClouds();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    for (let i = 0; i < 16; i += 1) {
      renderer.render(view, frameOf(planet, tables, EARTH_CLOUDS));
    }
    const labels = new Set(engine.targetFrames.map((f) => f.label));
    expect([labels.has(CLOUD_VOLUME_PASS), labels.has(CLOUD_LAYER_PASS)]).toEqual([false, true]);
    renderer.dispose();
    tables.dispose();
  });

  it("begins the volume's history again after a cut", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const air = frameOf(planet, tables, EARTH_CLOUDS);
    renderer.prepareClouds(air.atmospheres);
    const held = (): number | undefined =>
      engine.targetFrames.findLast((f) => f.label === CLOUD_RESOLVE_PASS)?.draws[0]?.uniforms[
        "camera"
      ]?.[3];
    renderer.render(view, air);
    renderer.render(view, air);
    const before = held();
    renderer.dropHistograms();
    renderer.render(view, air);
    expect([before, held()]).toEqual([1, 0]);
    renderer.dispose();
    tables.dispose();
  });
});

describe("withCloudsOnGround (R11.T7.b)", () => {
  const body = bodyAt("0200080020000000.0300", vec3(0, 0, -1e8));
  const other = bodyAt("0200080020000000.0301", vec3(0, 0, -2e8));
  const gpu = {
    layer: { kind: "buffer", name: "rows", bytes: 544 },
    coverage: { kind: "texture", name: "coverage" },
    noise: { kind: "texture", name: "noise" },
  } as const;
  const clouds: ReadonlyMap<string, FrameClouds> = new Map([
    [
      body.id,
      {
        condensate: {
          condensate: "H2O",
          phase: "liquid",
          asymmetry: 0.86,
          coAlbedo: 5e-7,
          inCloudOpticalDepth550: 8,
          baseAltitudeM: 1_000,
          topAltitudeM: 3_000,
          opticsSource: "stand-in",
          opticalDepthSource: "record",
        },
        gpu,
      },
    ],
  ]);

  it("gives each body its clouds where cloudShadows is on", () => {
    const [withClouds, without] = withCloudsOnGround([body, other], clouds, true);
    expect(withClouds?.clouds).toBe(gpu);
    expect(without).toBe(other);
  });

  it("writes neither hook with cloudShadows off: the bodies as they are", () => {
    const bodies = [body, other];
    expect(withCloudsOnGround(bodies, clouds, false)).toBe(bodies);
  });
});
