import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { sphereFootprint } from "../bodies/regime";
import {
  earthBody,
  fiveLineSlotMedium,
  fourAbsorberMedium,
  iceGiantMethaneBody,
  lineAbsorberMedium,
  r05EarthWithOzoneAbsorber,
  SATURN_SPHEROID,
  saturnBody,
  saturnClassMedium,
  titanClassMethaneMedium,
  venusClassMedium,
  WGS84_SPHEROID,
} from "../../test/atmosphereFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import {
  BODY_DISC_CELLS_KERNEL,
  type BodyFramePlan,
  DISC_CELLS_PASS,
  type LitBodyInput,
} from "../bodies/draw";
import { packClassMap } from "../bodies/discSurface";
import { rotationColumns } from "../bodies/smoothMesh";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { Viewport } from "../camera/projection";
import { rotation3FromRows } from "../coords/rotation";
import { TIMING_FRAMES_IN_FLIGHT } from "../engine/webgpu/timing";
import type { DrawItem, FrameSubmission, RenderView, TextureHandle } from "../engine/types";
import { AU_M } from "../scenes/kept";
import { BloomChain } from "../post/bloomChain";
import { HISTOGRAM_KERNEL, histogramParams, histogramWorkgroups } from "../post/histogram";
import type { QualitySetting } from "../quality/qualitySetting";
import { limbDistanceM, pixelAngleRad } from "../terrain/handover";
import { OUTSIDE_VIEW_MARGIN_PX, type SurveyedDatumFrame } from "../wireframe/submit";
import {
  AERIAL_PERSPECTIVE_KERNEL,
  ATMOSPHERE_COMPOSITE_PASS,
  AtmosphereRetired,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  AERIAL_PERSPECTIVE_THICK_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_KERNEL,
  RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  RAY_MARCH_THICK_SPECIAL_KERNEL,
  SKY_VIEW_KERNEL,
  SKY_VIEW_THICK_KERNEL,
  tableRadiusM,
} from "../atmosphere/hillaire";
import { EngineUnavailable } from "../engine/resilientEngine";
import { R05_EARTH_REFERENCE } from "../atmosphere/earth";
import type { AtmosphereMedium } from "../atmosphere/medium";
import {
  type AbsorberCurveSlots,
  type AtmosphereBody,
  AtmosphereTables,
  IRRADIANCE_KERNEL,
  kernelMedium,
  MULTI_SCATTERING_KERNEL,
  SLOT_DEPTHS_KERNEL,
  TRANSMITTANCE_KERNEL,
} from "../atmosphere/tables";
import type { KernelPair } from "../engine/kernels";
import { kernelKey, prepareKernel } from "../engine/webgpu/compute";
import { BAKE_KERNELS } from "../sky/bake";
import { between, seededRandom } from "../../test/seededRandom";
import { patternedThickBake } from "../../test/thickTables";
import {
  type FrameAtmosphere,
  type PhotorealFrame,
  lostDevice,
  type PhotorealOverlay,
  PhotorealRenderer,
} from "./renderer";
import { TONEMAP_PASS } from "../post/tonemap";
import { PHOTOREAL_PASS_LABELS, SKY_PASS_LABEL } from "./passes";

const VIEWPORT = { widthPx: 64, heightPx: 36 };

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

function frameWith(sky: ReadonlyArray<DrawItem>): PhotorealFrame {
  const lit = aLitBody();
  return {
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    role: "eye",
    setting: "high",
    exposureScale: 1e-4,
    sky,
    starSprites: [[10, 10, 0, 0, 1, 1, 1, 0]],
    hostDraws: new Map(),
    glareSources: [],
    lights: [{ disc: aHostDisc(), centreM: vec3(AU_M, 0, -1e8) }],
    bodies: [
      {
        id: lit.body,
        centreM: vec3(0, 0, -1e8),
        figure: lit.figure,
        photometry: lit.photometry,
        lighting: undefined,
      },
    ],
    depthWriters: [],
    previousRegimes: new Map(),
    overlay: null,
    meter: "average",
  };
}

describe("the photorealistic renderer", () => {
  it("draws nothing until its pipelines are made", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    expect(renderer.render(view, frameWith([]))).toBeNull();
    renderer.dispose();
  });

  it("draws the sky and the stars, then the bodies over them, then tones onto the canvas", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const band: DrawItem = {
      mesh: { kind: "mesh", name: "band triangle" },
      material: { kind: "material", name: "sky:band" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    expect(renderer.render(view, frameWith([band]))).not.toBeNull();
    const passes = engine.targetFrames.map((f) => [
      f.label,
      f.colourLoad ?? "clear",
      f.draws.map((draw) => draw.material.name),
    ]);
    expect(passes).toEqual([
      [SKY_PASS_LABEL, "clear", ["sky:band", "sky:starSpriteHdr"]],
      [PHOTOREAL_PASS_LABELS.discs, "load", ["bodies:disc", "bodies:discLimb"]],
      ...passes.slice(2),
    ]);
    expect(view.frames.map((f) => [f.label, f.encoding])).toEqual([["tonemap", "in-pass"]]);
    renderer.dispose();
  });

  it("draws mesh bodies with depth in the bodies pass, between the sky and the painter's sequence", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const frame = frameWith([]);
    const [planet] = frame.bodies;
    if (planet === undefined) {
      throw new Error("no body");
    }
    // A depth writer over the planet, standing for R10's terrain.
    const writer = sphereFootprint(
      planet.centreM,
      planet.figure.equatorialRadiusM,
      camera,
      VIEWPORT,
    );
    const plan = renderer.render(view, { ...frame, depthWriters: writer === null ? [] : [writer] });
    expect(plan?.regimes.get(planet.id)).toBe("mesh");
    const passes = engine.targetFrames.map((f) => [
      f.label,
      f.colourLoad ?? "clear",
      f.draws.map((draw) => draw.material.name),
    ]);
    expect(passes.slice(0, 3)).toEqual([
      [SKY_PASS_LABEL, "clear", ["sky:starSpriteHdr"]],
      [PHOTOREAL_PASS_LABELS.bodies, "load", ["bodies:smoothMesh"]],
      [PHOTOREAL_PASS_LABELS.discs, "load", ["bodies:discLimb"]],
    ]);
    renderer.dispose();
  });

  it("draws a body handed over as a mesh, and its terrain after its figure in the bodies pass where the frame has one (R10.T10.e)", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const frame = frameWith([]);
    // The planet's relief subtends 4 px at its limb, 10⁸ m off: handed over.
    const bodies = frame.bodies.map((body) => ({
      ...body,
      handoverReliefM:
        4 * limbDistanceM(1e8, body.figure.equatorialRadiusM) * pixelAngleRad(camera, VIEWPORT),
    }));
    const terrainDraw: DrawItem = {
      mesh: { kind: "mesh", name: "terrain patches" },
      material: { kind: "material", name: "terrain-lit-baked-offsets" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    const asked: BodyFramePlan[] = [];
    const terrain = {
      surveyCoverage: () => null,
      draws: (plan: BodyFramePlan) => {
        asked.push(plan);
        return plan.meshes.some((mesh) => mesh.role.kind === "handed-over") ? [terrainDraw] : [];
      },
      symbologyDepth: () => [],
    };
    const bodiesPass = (from: number): ReadonlyArray<string> | undefined =>
      engine.targetFrames
        .slice(from)
        .find((f) => f.label === PHOTOREAL_PASS_LABELS.bodies)
        ?.draws.map((draw) => draw.material.name);
    // Without a terrain the body is drawn as a mesh body is.
    const bare = renderer.render(new RecordingView(), { ...frame, bodies });
    expect(bare?.regimes.get(bodies[0]?.id ?? "")).toBe("terrain");
    expect(bodiesPass(0)).toEqual(["bodies:smoothMesh"]);
    const from = engine.targetFrames.length;
    const plan = renderer.render(new RecordingView(), {
      ...frame,
      bodies,
      previousRegimes: bare?.regimes ?? new Map(),
      terrain,
    });
    expect(asked).toEqual([plan]);
    expect(bodiesPass(from)).toEqual(["bodies:smoothMesh", "terrain-lit-baked-offsets"]);
    renderer.dispose();
  });

  it("lights each body by planetshine from two neighbours on the high setting and one on the low", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    const frame = frameWith([]);
    const [planet] = frame.bodies;
    if (planet === undefined) {
      throw new Error("no body");
    }
    // Two moons on the planet's night side, full from it and clear of its shadow.
    const moonFigure = { equatorialRadiusM: 1.737e6, polarRadiusM: 1.737e6, pole: null };
    const moons = [
      {
        ...planet,
        id: "0200080020000000.0301",
        centreM: vec3(-4e8, -3e7, -1e8),
        figure: moonFigure,
      },
      {
        ...planet,
        id: "0200080020000000.0302",
        centreM: vec3(-4e8, 5e7, -1e8),
        figure: moonFigure,
      },
    ];
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    const sources = (["high", "low"] as const).map((setting) => {
      const plan = renderer.render(view, { ...frame, setting, bodies: [planet, ...moons] });
      return plan?.discs.find((disc) => disc.body === planet.id)?.secondaries.length;
    });
    expect(sources).toEqual([2, 1]);
    renderer.dispose();
  });

  it("makes its resources again after a device restore, and draws after them", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    engine.restore();
    expect(renderer.status).toBe("idle");
    expect(renderer.render(view, frameWith([]))).toBeNull();
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    expect([renderer.status, renderer.render(view, frameWith([])) !== null]).toEqual([
      "ready",
      true,
    ]);
    renderer.dispose();
  });

  it("abandons a making a restore overtakes", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    const first = renderer.prepare(VIEWPORT, "high", "eye", camera);
    engine.restore();
    await first;
    expect(renderer.status).toBe("idle");
    renderer.dispose();
  });

  it("releases its buffers and textures when disposed", async () => {
    const engine = await countingRenderEngine();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    renderer.dispose();
    expect(released).toEqual(
      expect.arrayContaining([
        "test view glare sources",
        "test view star sprites",
        "test view blue noise",
      ]),
    );
  });
});

/** Earth's mean radius, m: R05's spherical Earth, on which R05's medium's tables are built. */
const EARTH_RADIUS_M = 6.371e6;

/** A body of the frame at `centreM` from the camera, with `id`. */
function bodyAt(id: string, centreM: ReturnType<typeof vec3>): LitBodyInput {
  const lit = aLitBody();
  return { id, centreM, figure: lit.figure, photometry: lit.photometry, lighting: undefined };
}

/**
 * A body's atmosphere in the frame over `tables`, its camera from the body's centre along the
 * galactic axes, which are its body-fixed ones here.
 */
function airOf(body: LitBodyInput, tables: AtmosphereTables): FrameAtmosphere {
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
  };
}

/** A prepared renderer on the counting engine, with a view recording its canvas frames. */
async function preparedRenderer(): Promise<{
  readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
  readonly view: RecordingView;
  readonly renderer: PhotorealRenderer;
}> {
  const engine = await countingRenderEngine();
  const view = new RecordingView();
  const renderer = new PhotorealRenderer(engine, "test view");
  await renderer.prepare(VIEWPORT, "high", "eye", {
    orientation: IDENTITY_QUATERNION,
    fovXRad: Math.PI / 3,
  });
  return { engine, view, renderer };
}

describe("the photorealistic renderer's atmosphere (R08.T10.b)", () => {
  it("submits no atmosphere for a view whose bodies have no medium", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    renderer.render(view, frameWith([]));
    expect(engine.targetFrames.map((f) => f.label)).not.toContain(ATMOSPHERE_COMPOSITE_PASS);
    expect(engine.dispatched.map((d) => d.kernel)).not.toContain("atmosphere ray march");
    renderer.dispose();
  });

  it("draws a body's composite after the discs, over the scene target, for the passes after to read", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -3e7));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, {
      ...frameWith([]),
      bodies: [planet],
      atmospheres: [airOf(planet, tables)],
    });
    const labels = engine.targetFrames.map((f) => f.label);
    const at = labels.indexOf(ATMOSPHERE_COMPOSITE_PASS);
    expect(labels.slice(at - 1, at + 1)).toEqual([
      PHOTOREAL_PASS_LABELS.discs,
      ATMOSPHERE_COMPOSITE_PASS,
    ]);
    const composite = engine.targetFrames[at]?.draws[0];
    const scene = engine.targetFrames[0]?.draws;
    expect(composite?.material.name).toBe("atmosphere composite");
    expect(scene).toBeDefined();
    // The tone-mapping pass reads the composite's target, not the scene target the sky drew in.
    const tonemapped = view.frames[0]?.draws[0]?.textures["hdrColour"];
    expect(tonemapped).not.toBe(composite?.textures["sceneColour"]);
    renderer.dispose();
    tables.dispose();
  });

  it("lights a body with air through its tables and draws it with depth, as a mesh", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -3e7));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const plan = renderer.render(view, {
      ...frameWith([]),
      bodies: [planet],
      atmospheres: [airOf(planet, tables)],
    });
    expect(plan?.regimes.get(planet.id)).toBe("mesh");
    expect(plan?.discs[0]?.atmosphere?.transmittance).toBe(tables.transmittance);
    renderer.dispose();
    tables.dispose();
  });

  it("draws an air body's mesh in the bodies pass before its composite (decision-r08-t10b-depth §6)", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -3e7));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, {
      ...frameWith([]),
      bodies: [planet],
      atmospheres: [airOf(planet, tables)],
    });
    const passes = engine.targetFrames.map((f) => [
      f.label,
      f.draws.map((draw) => draw.material.name),
    ]);
    const bodies = passes.findIndex(([label]) => label === PHOTOREAL_PASS_LABELS.bodies);
    const composite = passes.findIndex(([label]) => label === ATMOSPHERE_COMPOSITE_PASS);
    expect(passes[bodies]?.[1]).toContain("bodies:smoothMesh");
    expect(bodies).toBeGreaterThanOrEqual(0);
    expect(bodies).toBeLessThan(composite);
    renderer.dispose();
    tables.dispose();
  });

  it("submits no composite for an air body drawn as a point", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    // 6,371 km at 1.5 × 10¹¹ m spans well under 3 px of a 64 px view.
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -1.5e11));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const plan = renderer.render(view, {
      ...frameWith([]),
      bodies: [planet],
      atmospheres: [airOf(planet, tables)],
    });
    expect(plan?.regimes.get(planet.id)).toBe("point");
    expect(engine.targetFrames.map((f) => f.label)).not.toContain(ATMOSPHERE_COMPOSITE_PASS);
    renderer.dispose();
    tables.dispose();
  });

  it("lays a handed-over body's air over its terrain, which takes its record's tables (R10.T16.c's binding)", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    const planet = {
      ...bodyAt("0200080020000000.0300", vec3(0, 0, -1e8)),
      handoverReliefM: 4 * limbDistanceM(1e8, 6.371e6) * pixelAngleRad(camera, VIEWPORT),
    };
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const terrainDraw: DrawItem = {
      mesh: { kind: "mesh", name: "terrain patches" },
      material: { kind: "material", name: "terrain-lit-baked-offsets" },
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    const lit: BodyFramePlan[] = [];
    const terrain = {
      surveyCoverage: () => null,
      draws: (plan: BodyFramePlan) => {
        lit.push(plan);
        return plan.meshes.some((mesh) => mesh.role.kind === "handed-over") ? [terrainDraw] : [];
      },
      symbologyDepth: () => [],
    };
    const air = { ...frameWith([]), bodies: [planet], atmospheres: [airOf(planet, tables)] };
    const first = renderer.render(view, air);
    const from = engine.targetFrames.length;
    renderer.render(view, { ...air, previousRegimes: first?.regimes ?? new Map(), terrain });
    const labels = engine.targetFrames.slice(from).map((f) => f.label);
    const bodiesAt = labels.indexOf(PHOTOREAL_PASS_LABELS.bodies);
    expect(engine.targetFrames[from + bodiesAt]?.draws.map((d) => d.material.name)).toContain(
      "terrain-lit-baked-offsets",
    );
    expect(bodiesAt).toBeLessThan(labels.indexOf(ATMOSPHERE_COMPOSITE_PASS));
    const record = lit.at(-1)?.discs.find((each) => each.body === planet.id);
    expect(record?.atmosphere?.transmittance).toBe(tables.transmittance);
    renderer.dispose();
    tables.dispose();
  });

  it("draws three bodies' composites, none into the target it reads", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const bodies = [
      bodyAt("0200080020000000.0300", vec3(-3e7, 0, -6e7)),
      bodyAt("0200080020000000.0400", vec3(0, 0, -8e7)),
      bodyAt("0200080020000000.0500", vec3(3e7, 0, -1e8)),
    ];
    const tables = bodies.map(
      () => new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M),
    );
    renderer.render(view, {
      ...frameWith([]),
      bodies,
      atmospheres: bodies.map((body, i) => {
        const own = tables[i];
        if (own === undefined) {
          throw new Error("a body without its tables");
        }
        return airOf(body, own);
      }),
    });
    const composites = engine.targetFrames.filter((f) => f.label === ATMOSPHERE_COMPOSITE_PASS);
    expect(composites).toHaveLength(3);
    // Each composite reads the colour the one before it drew, the first the scene's.
    const reads = composites.map((f) => f.draws[0]?.textures["sceneColour"]?.name);
    expect(new Set(reads.slice(1)).size).toBe(2);
    expect(reads[1]).not.toBe(reads[0]);
    expect(reads[2]).not.toBe(reads[1]);
    renderer.dispose();
    for (const each of tables) {
      each.dispose();
    }
  });

  it("draws two bodies' composites farthest first, the nearer over the farther", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const near = bodyAt("0200080020000000.0300", vec3(-2e7, 0, -4e7));
    const far = bodyAt("0200080020000000.0400", vec3(2e7, 0, -8e7));
    const nearTables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    const farTables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, {
      ...frameWith([]),
      bodies: [near, far],
      atmospheres: [airOf(near, nearTables), airOf(far, farTables)],
    });
    const composites = engine.targetFrames.filter((f) => f.label === ATMOSPHERE_COMPOSITE_PASS);
    expect(composites).toHaveLength(2);
    const [first, second] = composites;
    expect(first?.draws[0]?.textures["transmittance"]).toBe(farTables.transmittance);
    expect(second?.draws[0]?.textures["transmittance"]).toBe(nearTables.transmittance);
    // The nearer composite reads what the farther one drew.
    expect(second?.draws[0]?.textures["sceneColour"]).not.toBe(
      first?.draws[0]?.textures["sceneColour"],
    );
    renderer.dispose();
    nearTables.dispose();
    farTables.dispose();
  });

  it("lets a body's atmosphere go when the frame no longer draws it, and never disposes the cache's tables", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -3e7));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, {
      ...frameWith([]),
      bodies: [planet],
      atmospheres: [airOf(planet, tables)],
    });
    renderer.render(view, { ...frameWith([]), bodies: [planet] });
    expect(released).toEqual(
      expect.arrayContaining(["atmosphere sky view", "atmosphere aerial perspective"]),
    );
    renderer.dispose();
    expect(released).not.toContain(tables.transmittance.name);
    expect(() => {
      tables.setMedium(R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    }).not.toThrow();
    tables.dispose();
  });

  it("releases the atmospheres' composite and its triangle with the view", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const materials = vi.spyOn(engine, "releaseMaterial");
    const meshes = vi.spyOn(engine, "releaseMesh");
    const planet = bodyAt("0200080020000000.0300", vec3(0, 0, -3e7));
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
    renderer.render(view, {
      ...frameWith([]),
      bodies: [planet],
      atmospheres: [airOf(planet, tables)],
    });
    renderer.dispose();
    tables.dispose();
    expect([
      materials.mock.calls.filter(([material]) => material.name === "atmosphere composite").length,
      meshes.mock.calls.filter(([mesh]) => mesh.name === "atmosphere composite triangle").length,
    ]).toEqual([1, 1]);
  });

  describe("a body's air that throws as it is drawn (decision-r08-t10b-leans §2)", () => {
    it("reports the body once over several frames, and draws the other body's air and the frame's passes", async () => {
      const { engine, view, renderer } = await preparedRenderer();
      const broken = bodyAt("0200080020000000.0300", vec3(-2e7, 0, -4e7));
      const sound = bodyAt("0200080020000000.0400", vec3(2e7, 0, -8e7));
      const brokenTables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
      const soundTables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
      // A view wider than the device's textures: its ray march's target cannot be made.
      const tooWide = airOf(broken, brokenTables);
      const wide = {
        ...tooWide,
        camera: { ...tooWide.camera, viewport: { widthPx: 1e6, heightPx: 36 } },
      };
      const reported: Array<readonly [string, boolean, string]> = [];
      for (let frame = 0; frame < 3; frame += 1) {
        // The frame's tables are the cache's, which holds a reported body failed and hands none.
        const failed = new Set(reported.map(([body]) => body));
        renderer.render(view, {
          ...frameWith([]),
          bodies: [broken, sound],
          atmospheres: [wide, airOf(sound, soundTables)].filter((air) => !failed.has(air.id)),
          onAtmosphereFailed: (body, error, viewName) => {
            reported.push([body, error instanceof Error, viewName]);
          },
        });
      }
      expect(reported).toEqual([[broken.id, true, "test view"]]);
      const composites = engine.targetFrames.filter((f) => f.label === ATMOSPHERE_COMPOSITE_PASS);
      expect(composites.map((f) => f.draws[0]?.textures["transmittance"])).toEqual(
        Array.from({ length: 3 }, () => soundTables.transmittance),
      );
      // Each frame is still tone-mapped onto the canvas.
      expect(view.frames.filter((f) => f.label === TONEMAP_PASS)).toHaveLength(3);
      renderer.dispose();
      brokenTables.dispose();
      soundTables.dispose();
    });

    it("reports nothing for a lost device's throw, which the graphics fault covers", async () => {
      const { engine, view, renderer } = await preparedRenderer();
      const seen = bodyAt("0200080020000000.0300", vec3(-2e7, 0, -4e7));
      const next = bodyAt("0200080020000000.0400", vec3(2e7, 0, -8e7));
      const seenTables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
      const nextTables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
      const failed = vi.fn<(body: string, error: unknown, viewName: string) => void>();
      const logged = vi.spyOn(console, "error").mockImplementation(() => undefined);
      // The first frame makes everything the second takes but the next body's ray march target,
      // whose view is too wide for the device's textures.
      const tooWide = airOf(next, nextTables);
      renderer.render(view, {
        ...frameWith([]),
        bodies: [seen, next],
        atmospheres: [
          airOf(seen, seenTables),
          { ...tooWide, camera: { ...tooWide.camera, viewport: { widthPx: 1e6, heightPx: 36 } } },
        ],
        onAtmosphereFailed: failed,
      });
      const before = failed.mock.calls.length;
      // That target is made inside the loss's window, where every creation throws.
      engine.loseDevice();
      renderer.render(view, {
        ...frameWith([]),
        bodies: [seen, next],
        atmospheres: [airOf(seen, seenTables), airOf(next, nextTables)],
        onAtmosphereFailed: failed,
      });
      expect([before, failed.mock.calls.length, logged.mock.calls.length]).toEqual([1, 1, 0]);
      engine.restore();
      renderer.dispose();
      seenTables.dispose();
      nextTables.dispose();
    });

    it("tells a lost device's throws by their type", () => {
      expect([
        lostDevice(new EngineUnavailable("createTexture")),
        lostDevice(new AtmosphereRetired()),
        lostDevice(new RangeError("the device has no device")),
      ]).toEqual([true, true, false]);
    });
  });

  describe("a body's air whose figure is off the view (decision-r08-t10b-leans §3)", () => {
    /**
     * A body beside the view's right edge at 3 × 10⁷ m ahead, its centre `beyondM` past the edge's
     * plane, the margin `sphereOutsideView` allows included.
     */
    function besideView(beyondM: number): LitBodyInput {
      const tanHalf = Math.tan(Math.PI / 6);
      const tanX = tanHalf + (OUTSIDE_VIEW_MARGIN_PX * 2 * tanHalf) / VIEWPORT.widthPx;
      const aheadM = 3e7;
      return bodyAt(
        "0200080020000000.0300",
        vec3(tanX * aheadM + beyondM * Math.hypot(1, tanX), 0, -aheadM),
      );
    }

    /** The composites drawn for a body `beyondM` past the view's edge. */
    async function compositesBeside(beyondM: number): Promise<number> {
      const { engine, view, renderer } = await preparedRenderer();
      const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_RADIUS_M);
      const planet = besideView(beyondM);
      renderer.render(view, {
        ...frameWith([]),
        bodies: [planet],
        atmospheres: [airOf(planet, tables)],
      });
      const count = engine.targetFrames.filter((f) => f.label === ATMOSPHERE_COMPOSITE_PASS).length;
      renderer.dispose();
      tables.dispose();
      return count;
    }

    it("draws its composite while its shell meets the view", async () => {
      const topM = R05_EARTH_REFERENCE.topHeightM;
      expect(await compositesBeside(EARTH_RADIUS_M + topM / 2)).toBe(1);
    });

    it("draws none once its shell is off the view too", async () => {
      const topM = R05_EARTH_REFERENCE.topHeightM;
      expect(await compositesBeside(EARTH_RADIUS_M + 2 * topM)).toBe(0);
    });
  });
});

/** A draw of `material`, by its name alone. */
function drawOf(material: string): DrawItem {
  return {
    mesh: { kind: "mesh", name: "mesh" },
    material: { kind: "material", name: material },
    offsetFromCameraM: new Float32Array(3),
    uniforms: {},
    textures: {},
  };
}

/** An empty symbology pass, as `overlaySubmission` labels it. */
const SYMBOLOGY: FrameSubmission = {
  label: PHOTOREAL_PASS_LABELS.symbology,
  viewRotation: new Float32Array(16),
  projection: new Float32Array(16),
  draws: [],
  postProcesses: [],
};

/** A coverage mask's handle, as `CoverageMask.texture` gives it. */
const MASK: TextureHandle = { kind: "texture", name: "coverage mask" };

/** A rotation with no axis in common with the galactic ones but z: a quarter turn about it. */
const QUARTER_TURN = rotation3FromRows([vec3(0, 1, 0), vec3(-1, 0, 0), vec3(0, 0, 1)]);

/**
 * The frame's planet, oblate (a ÷ c = 1.0034) and turning by {@link QUARTER_TURN}, its relief 4 px
 * at its limb 10⁸ m off, so that it is handed over to terrain; with its survey's mask in that
 * rotation unless `coverage` is false.
 */
function handedOverFrame(coverage = true): PhotorealFrame {
  const frame = frameWith([]);
  const [planet] = frame.bodies;
  if (planet === undefined) {
    throw new Error("no body");
  }
  const { equatorialRadiusM } = planet.figure;
  const body: LitBodyInput = {
    ...planet,
    figure: { ...planet.figure, polarRadiusM: equatorialRadiusM / 1.0034 },
    rotation: QUARTER_TURN,
    handoverReliefM:
      4 * limbDistanceM(1e8, equatorialRadiusM) * pixelAngleRad(frame.camera, VIEWPORT),
  };
  return {
    ...frame,
    bodies: [coverage ? { ...body, surveyCoverage: { mask: MASK, rotation: QUARTER_TURN } } : body],
  };
}

/**
 * Renders `frame` with an overlay that records the datums the renderer builds it with, and gives
 * the plan, whether the overlay was drawn, and the datums.
 */
async function overlayDatums(
  frame: PhotorealFrame,
  prepared = true,
): Promise<{
  readonly plan: BodyFramePlan | null;
  readonly symbology: boolean;
  readonly given: ReadonlyArray<ReadonlyMap<string, SurveyedDatumFrame>>;
}> {
  const engine = await countingRenderEngine();
  const view = new RecordingView();
  const renderer = new PhotorealRenderer(engine, "test view");
  if (prepared) {
    await renderer.prepare(VIEWPORT, "high", "eye", frame.camera);
  }
  const given: Array<ReadonlyMap<string, SurveyedDatumFrame>> = [];
  const overlay: PhotorealOverlay = (surveyed) => {
    given.push(surveyed);
    return SYMBOLOGY;
  };
  const plan = renderer.render(view, { ...frame, overlay });
  renderer.dispose();
  return {
    plan,
    symbology: view.frames.some((f) => f.label === PHOTOREAL_PASS_LABELS.symbology),
    given,
  };
}

/**
 * Every kernel VIEW makes on a device (plan R01, R01.T15): the photorealistic renderer's histogram
 * and disc cells; the atmosphere's per-frame kernels, its sky view, aerial perspective, its two
 * marches' footprint entry points, each general and specialised for one band and no layer edge
 * (R08.T11.d), their centre ray's entry points under R08.T11.d's tile cull (off for a view) and the
 * march's fill; since R08.T14.c the sky view, the aerial perspective and both marches' footprints,
 * general and specialised, reading the thick table (`withThickRead`), for a body whose tables hold
 * one; its per-planet tables' four; and the sky bake's four. Each is a module constant, its
 * WGSL fixed text with no value of a medium, a body or a setting in it (term counts, slices, bands
 * and light slots are uniforms; the specialised march and the thick read are variants of fixed
 * text, chosen from the tables), so the records a device's cache holds for VIEW are bounded by this
 * list's length, 25, however many bodies a session visits. The smoke's kernel arm (`withKernelArmFormat`, twelve more)
 * and its probes are the smoke's alone.
 */
const VIEW_KERNELS: ReadonlyArray<KernelPair> = [
  HISTOGRAM_KERNEL,
  BODY_DISC_CELLS_KERNEL,
  SKY_VIEW_KERNEL,
  AERIAL_PERSPECTIVE_KERNEL,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  SKY_VIEW_THICK_KERNEL,
  AERIAL_PERSPECTIVE_THICK_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  RAY_MARCH_SLOTS_THICK_KERNEL,
  RAY_MARCH_THICK_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
  TRANSMITTANCE_KERNEL,
  MULTI_SCATTERING_KERNEL,
  SLOT_DEPTHS_KERNEL,
  IRRADIANCE_KERNEL,
  ...BAKE_KERNELS,
];

/** A body's air as the optics pool builds it, for the app's tables factory. */
interface SessionAir {
  readonly medium: AtmosphereMedium;
  readonly bottomRadiusM: number;
  readonly curves: AbsorberCurveSlots;
  readonly body: AtmosphereBody | null;
}

/** A kind of air, and whether it stands on an Earth-sized ground that WGS 84's figure may slice. */
interface AirKind {
  readonly air: SessionAir;
  readonly earthLike: boolean;
}

/** Air on an Earth-sized ground, WGS 84's mean radius, with no figure of its own. */
function onEarth(medium: AtmosphereMedium, curves: AbsorberCurveSlots): AirKind {
  return {
    air: { medium, bottomRadiusM: tableRadiusM(WGS84_SPHEROID), curves, body: null },
    earthLike: true,
  };
}

/**
 * The fixtures' kinds of air, each built once (their curves of growth are most of a build's cost):
 * R05's Earth, with and without its ozone as an absorber, four column absorbers under one, two and
 * three seeded lights, a line absorber, five line slots, a Venus-class deck, a Titan-class methane
 * air, an oblate ice giant and an oblate Saturn-class air; so they differ in their terms,
 * absorbers, light slots, slices and bands.
 */
function airKinds(random: () => number): AirKind[] {
  const ozone = r05EarthWithOzoneAbsorber();
  const lines = lineAbsorberMedium();
  const slots = fiveLineSlotMedium();
  const venus = venusClassMedium();
  const titan = titanClassMethaneMedium();
  const iceGiant = iceGiantMethaneBody();
  return [
    onEarth(R05_EARTH_REFERENCE, []),
    onEarth(ozone.medium, ozone.curves),
    ...[1, 2, 3].map((count) => {
      const lightsK = Array.from({ length: count }, () => between(random, 3_000, 7_000));
      const { medium, curves } = fourAbsorberMedium(lightsK);
      return onEarth(medium, curves);
    }),
    onEarth(lines.medium, lines.curves),
    onEarth(slots.medium, slots.curves),
    {
      air: { medium: venus.medium, bottomRadiusM: venus.radiusM, curves: [], body: null },
      earthLike: false,
    },
    {
      air: { medium: titan.medium, bottomRadiusM: titan.radiusM, curves: titan.curves, body: null },
      earthLike: false,
    },
    {
      air: {
        medium: iceGiant.medium,
        bottomRadiusM: tableRadiusM(iceGiant.body.figure),
        curves: iceGiant.curves,
        body: iceGiant.body,
      },
      earthLike: false,
    },
    {
      air: {
        medium: saturnClassMedium(),
        bottomRadiusM: tableRadiusM(SATURN_SPHEROID),
        curves: [],
        body: saturnBody(),
      },
      earthLike: false,
    },
  ];
}

/**
 * `count` bodies' air, as a session meets them: `kinds` in turn, each body's medium its own (its
 * name and a seeded grey ground albedo), and an Earth-like kind on WGS 84's figure and gravity
 * every second turn.
 */
function sessionAirs(
  kinds: ReadonlyArray<AirKind>,
  count: number,
  random: () => number,
): SessionAir[] {
  return Array.from({ length: count }, (_, i) => {
    const kind = kinds[i % kinds.length];
    if (kind === undefined) {
      throw new Error("no kind of air");
    }
    const albedo = between(random, 0.05, 0.5);
    const sliced = kind.earthLike && Math.floor(i / kinds.length) % 2 === 1;
    return {
      ...kind.air,
      medium: {
        ...kind.air.medium,
        name: `${kind.air.medium.name} ${i}`,
        groundAlbedo: [albedo, albedo, albedo],
      },
      body: sliced ? earthBody() : kind.air.body,
    };
  });
}

describe("the device's kernels over a session's bodies (R01.T15)", () => {
  it("stay within VIEW's fixed set, and stop growing after the first of each kind of air, thin and thick, over 50 bodies", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const keyOf = (pair: KernelPair): string =>
      kernelKey(pair, prepareKernel(pair, engine.capabilities));
    // The distinct kernels made so far, read incrementally: a key holds the whole WGSL text.
    const distinct = new Set<string>();
    let counted = 0;
    const count = (): number => {
      for (const pair of engine.kernelPairs.slice(counted)) {
        distinct.add(keyOf(pair));
      }
      counted = engine.kernelPairs.length;
      return distinct.size;
    };
    const random = seededRandom(15);
    const kinds = airKinds(random);
    const airs = sessionAirs(kinds, 50, random);
    const held: number[] = [];
    for (const [i, air] of airs.entries()) {
      const planet = bodyAt(
        `0200080020000000.${String(300 + i).padStart(4, "0")}`,
        vec3(0, 0, -3e7),
      );
      // The app's factory's construction (`atmosphereTablesOn`), and its eviction's disposal.
      const tables = new AtmosphereTables(
        engine,
        air.medium,
        air.bottomRadiusM,
        air.curves,
        air.body,
      );
      // The second pass over the kinds has its bodies' thick bakes landed (R08.T14.c).
      if (Math.floor(i / kinds.length) === 1) {
        tables.setThickTables(
          patternedThickBake(
            tables.plan.bands,
            kernelMedium(tables.medium),
            Math.max(1, air.curves.length),
          ),
        );
      }
      renderer.render(view, {
        ...frameWith([]),
        bodies: [planet],
        atmospheres: [airOf(planet, tables)],
      });
      tables.dispose();
      held.push(count());
    }
    const bound = new Set(VIEW_KERNELS.map(keyOf));
    expect(bound.size).toBe(VIEW_KERNELS.length);
    expect([...distinct].filter((key) => !bound.has(key))).toEqual([]);
    // Every kind of air has been met by its first body thin, and by its first thick (the second
    // pass); none after adds a kernel. The thin first pass makes 12, and the thick pass 6 more, 18
    // in all, bounded however many bodies follow. All but seven are made: the sky bake's four are
    // the sky's, the march's fill a reader's, which no frame asks, and the tile cull's two centre
    // ray's kernels its own cover's, which a view never takes (R08.T11.d).
    expect(new Set(held.slice(0, kinds.length).slice(-1))).toEqual(
      new Set([VIEW_KERNELS.length - BAKE_KERNELS.length - 3 - 6]),
    );
    expect(new Set(held.slice(2 * kinds.length - 1))).toEqual(new Set([held.at(-1)]));
    expect(held.at(-1)).toBe(VIEW_KERNELS.length - BAKE_KERNELS.length - 3);
    // Each body's tables made their own handles all the while.
    expect(engine.counts.kernels).toBeGreaterThan(2 * airs.length);
    renderer.dispose();
  });
});

describe("the photorealistic renderer's overlay", () => {
  it("loads the tone-mapped image for the symbology", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    const overlay = (): FrameSubmission => SYMBOLOGY;
    renderer.render(view, { ...frameWith([]), overlay });
    expect(view.frames.map((f) => [f.label, f.colourLoad])).toEqual([
      ["tonemap", undefined],
      ["symbology", "load"],
    ]);
    renderer.dispose();
  });

  it("draws the terrain's depth into the symbology's pass before its marks (R10.T16.c)", async () => {
    const engine = await countingRenderEngine();
    const view = new RecordingView();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    const overlay = (): FrameSubmission => ({
      ...SYMBOLOGY,
      draws: [drawOf("occluderSphere"), drawOf("lines")],
    });
    const terrain = {
      surveyCoverage: () => null,
      draws: () => [],
      symbologyDepth: () => [drawOf("terrain-depth-baked-offsets")],
    };
    renderer.render(view, { ...frameWith([]), overlay, terrain });
    expect(
      view.frames
        .find((f) => f.label === PHOTOREAL_PASS_LABELS.symbology)
        ?.draws.map((draw) => draw.material.name),
    ).toEqual(["terrain-depth-baked-offsets", "occluderSphere", "lines"]);
    renderer.dispose();
  });

  it("builds the symbology from its plan, a body handed over with its mask giving way in the plan's rotation and figure (R10.T16.f)", async () => {
    const frame = handedOverFrame();
    const { plan, symbology, given } = await overlayDatums(frame);
    const [planet] = frame.bodies;
    const role = plan?.meshes[0]?.role;
    expect([
      role?.kind,
      role?.kind === "handed-over" ? role.coverage?.mask : null,
      symbology,
    ]).toEqual(["handed-over", MASK, true]);
    expect(given).toEqual([
      new Map([
        [
          planet?.id,
          {
            mask: MASK,
            axes: rotationColumns(QUARTER_TURN),
            stretch: 6.371e6 / (6.371e6 / 1.0034),
          },
        ],
      ]),
    ]);
  });

  it("gives no datum for a body drawn whole: a disc, or one handed over without a mask (R10.T16.f)", async () => {
    const disc = await overlayDatums(frameWith([]));
    const unmasked = await overlayDatums(handedOverFrame(false));
    expect([
      disc.plan?.meshes.length,
      unmasked.plan?.meshes[0]?.role,
      [...disc.given.map((each) => each.size), ...unmasked.given.map((each) => each.size)],
    ]).toEqual([0, { kind: "handed-over", coverage: null }, [0, 0]]);
  });

  it("builds no symbology while its pipelines are being made (R10.T16.f)", async () => {
    const { plan, symbology, given } = await overlayDatums(handedOverFrame(), false);
    expect([plan, symbology, given]).toEqual([null, false, []]);
  });
});

describe("the photorealistic renderer when a pipeline is refused", () => {
  it("stands failed, prepares nothing more and releases the bloom chain it made", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const engine = await countingRenderEngine();
    const chainDisposed = vi.spyOn(BloomChain.prototype, "dispose");
    vi.spyOn(engine, "createMaterialAsync").mockImplementation((spec) =>
      spec.name === "tone mapping"
        ? Promise.reject(new Error("refused"))
        : Promise.resolve({ kind: "material", name: spec.name }),
    );
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    // The chain's release follows its making, a few microtask turns later.
    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });
    expect(renderer.status).toBe("failed");
    const materials = engine.counts.materials;
    expect(renderer.render(new RecordingView(), frameWith([]))).toBeNull();
    expect(engine.counts.materials).toBe(materials);
    expect(chainDisposed).toHaveBeenCalled();
    renderer.dispose();
  });
});

/** Lets the read-backs settle. */
async function settled(): Promise<void> {
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });
}

describe("the photorealistic renderer's histogram", () => {
  async function ready(): Promise<{
    readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
    readonly renderer: PhotorealRenderer;
  }> {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    return { engine, renderer };
  }

  it("is dispatched once a frame with the operator's meter's weights", async () => {
    const { engine, renderer } = await ready();
    renderer.render(new RecordingView(), { ...frameWith([]), meter: "lit" });
    expect(
      engine.dispatched
        .filter((d) => d.pass === PHOTOREAL_PASS_LABELS.histogram)
        .map((d) => [d.pass, Array.from(d.bindings.uniforms["params"] ?? [])]),
    ).toEqual([["histogram", Array.from(histogramParams(VIEWPORT, "lit", 1))]]);
    renderer.dispose();
  });

  it("is not taken on a frame that meters nothing, as an instrument's (R07.T19.c)", async () => {
    const { engine, renderer } = await ready();
    renderer.render(new RecordingView(), { ...frameWith([]), meter: null });
    await settled();
    const histograms = engine.dispatched.filter((d) => d.pass === PHOTOREAL_PASS_LABELS.histogram);
    expect([histograms.length, renderer.takeHistogram()]).toEqual([0, undefined]);
    renderer.dispose();
  });

  it("is handed over once read back, under the frame's pre-exposure", async () => {
    const { renderer } = await ready();
    renderer.render(new RecordingView(), frameWith([]));
    await settled();
    expect([renderer.takeHistogram()?.preExposure, renderer.takeHistogram()]).toEqual([
      1e-4,
      undefined,
    ]);
    renderer.dispose();
  });

  it("is dropped by a device restore", async () => {
    const { engine, renderer } = await ready();
    renderer.render(new RecordingView(), frameWith([]));
    engine.restore();
    await settled();
    expect(renderer.takeHistogram()).toBeUndefined();
    renderer.dispose();
  });

  it("is dropped by a dispose, which releases the reader's buffers", async () => {
    const { engine, renderer } = await ready();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    renderer.render(new RecordingView(), frameWith([]));
    renderer.dispose();
    await settled();
    expect(renderer.takeHistogram()).toBeUndefined();
    expect(released.filter((name) => name.startsWith("test view histogram"))).toHaveLength(3);
  });
});

describe("the photorealistic renderer at each quality setting (R07.T17)", () => {
  /** A renderer made at `setting`, and one frame drawn at it with the operator's meter. */
  async function drawnAt(setting: QualitySetting): Promise<{
    readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
    readonly plan: BodyFramePlan | null;
  }> {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    const frame = { ...frameWith([]), setting };
    await renderer.prepare(VIEWPORT, setting, "eye", frame.camera);
    const plan = renderer.render(new RecordingView(), frame);
    renderer.dispose();
    return { engine, plan };
  }

  it.each([
    { setting: "high", stride: 1 },
    { setting: "low", stride: 2 },
  ] as const)(
    "takes the $setting setting's histogram at a stride of $stride",
    async ({ setting, stride }) => {
      const { engine } = await drawnAt(setting);
      expect(
        engine.dispatched
          .filter((d) => d.pass === PHOTOREAL_PASS_LABELS.histogram)
          .map((d) => [Array.from(d.bindings.uniforms["params"] ?? []), d.workgroups]),
      ).toEqual([
        [
          Array.from(histogramParams(VIEWPORT, "average", stride)),
          histogramWorkgroups(VIEWPORT, stride),
        ],
      ]);
    },
  );

  it.each([
    { setting: "high", levels: [1, 2, 3, 4, 5, 6] },
    { setting: "low", levels: [1, 2, 3, 4, 5] },
  ] as const)(
    "blooms over the $setting setting's mip levels $levels",
    async ({ setting, levels }) => {
      const { engine } = await drawnAt(setting);
      const down = engine.textureSpecs
        .map((spec) => /^test view:bloom down (\d+)/.exec(spec.name)?.[1])
        .filter((level) => level !== undefined)
        .map(Number);
      const bloomPasses = engine.targetFrames.filter(
        (f) => f.label === PHOTOREAL_PASS_LABELS.bloom,
      ).length;
      // Down to the coarsest level, then up to level 1.
      expect([down, bloomPasses]).toEqual([levels, 2 * levels.length - 1]);
    },
  );

  it.each([
    { setting: "high", annuli: 4 },
    { setting: "low", annuli: 3 },
  ] as const)(
    "eclipses each disc's star over the $setting setting's $annuli annuli",
    async ({ setting, annuli }) => {
      const { plan } = await drawnAt(setting);
      const counts = plan?.discs.flatMap((disc) =>
        disc.lights.flatMap((light) => light.annuli.map((set) => set.flux.length)),
      );
      expect(counts).toEqual([annuli, annuli, annuli]);
    },
  );
});

/**
 * An Earth-sized body `diameterPx` across on a view of `viewport` at 60°, `xPx` right of its
 * centre.
 */
function bodyAcross(
  diameterPx: number,
  xPx: number,
  id: string,
  viewport: Viewport = VIEWPORT,
): LitBodyInput {
  const lit = aLitBody();
  const pxPerRad = viewport.widthPx / (2 * Math.tan(Math.PI / 6));
  const distanceM = lit.figure.equatorialRadiusM / Math.sin(diameterPx / 2 / pxPerRad);
  return {
    id,
    centreM: vec3((xPx * distanceM) / pxPerRad, 0, -distanceM),
    figure: lit.figure,
    photometry: lit.photometry,
    lighting: undefined,
  };
}

/** A frame of `frameWith`'s sky and lights with these bodies, drawn as discs. */
function frameOfDiscs(bodies: ReadonlyArray<LitBodyInput>, viewport = VIEWPORT): PhotorealFrame {
  return {
    ...frameWith([]),
    viewport,
    presentedPx: viewport,
    bodies,
    previousRegimes: new Map(bodies.map((body) => [body.id, "disc"])),
  };
}

/** A 3.5 px disc (8 × 8) and a 13 px one (4 × 4) on `viewport`. */
function smallDiscs(viewport: Viewport = VIEWPORT): LitBodyInput[] {
  const third = viewport.widthPx / 4;
  return [
    bodyAcross(3.5, third, "0200080020000000.0401", viewport),
    bodyAcross(13, -third, "0200080020000000.0402", viewport),
  ];
}

describe("the photorealistic renderer's cell pass (R07.T8.d)", () => {
  it("shades the small discs' cells once, after the sky and before the discs", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera);
    const plan = renderer.render(new RecordingView(), frameOfDiscs(smallDiscs()));
    expect(plan?.discs.map((disc) => disc.interiorSamples)).toEqual([8, 4]);
    const cells = engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS);
    expect(cells.map((d) => [d.kernel, engine.targetFrames[d.framesBefore - 1]?.label])).toEqual([
      [BODY_DISC_CELLS_KERNEL.name, SKY_PASS_LABEL],
    ]);
    expect(engine.targetFrames[cells[0]?.framesBefore ?? -1]?.label).toBe(
      PHOTOREAL_PASS_LABELS.discs,
    );
    renderer.dispose();
  });

  it("dispatches a disc of 32 px or more once, for its limb (R07.T8.f)", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera);
    const plan = renderer.render(
      new RecordingView(),
      frameOfDiscs([bodyAcross(34, 0, "0200080020000000.0403")]),
    );
    const cells = engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS);
    expect([plan?.discs.map((disc) => disc.interiorSamples), cells.length]).toEqual([[1], 1]);
    renderer.dispose();
  });

  it("dispatches nothing where every body is a point", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera);
    const plan = renderer.render(new RecordingView(), {
      ...frameOfDiscs([]),
      bodies: [bodyAcross(1.5, 0, "0200080020000000.0404")],
    });
    expect([
      plan?.discs.length,
      engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS).length,
    ]).toEqual([0, 0]);
    renderer.dispose();
  });

  it("dispatches once a set of textures: a plain disc and a larger one's limb, and a class map's disc (R07.T8.e)", async () => {
    const engine = await countingRenderEngine();
    const renderer = new PhotorealRenderer(engine, "test view");
    await renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera);
    renderer.render(new RecordingView(), frameOfDiscs(severalSets()));
    const cells = engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS);
    expect(new Set(cells.map((d) => d.bindings.sampled["class_weights"]?.name))).toEqual(
      new Set(["bodies:no class map", "test class map"]),
    );
    expect(cells).toHaveLength(2);
    renderer.dispose();
  });

  it("dispatches once in each of three photorealistic views", async () => {
    const engine = await countingRenderEngine();
    const renderers = ["primary", "instrument 1", "instrument 2"].map(
      (name) => new PhotorealRenderer(engine, name),
    );
    await Promise.all(
      renderers.map((renderer) => renderer.prepare(VIEWPORT, "high", "eye", frameWith([]).camera)),
    );
    for (const renderer of renderers) {
      renderer.render(new RecordingView(), frameOfDiscs(smallDiscs()));
    }
    const cells = engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS);
    expect([cells.length, new Set(cells.map((d) => d.bindings.buffers["cell_jobs"])).size]).toEqual(
      [3, 3],
    );
    for (const renderer of renderers) {
      renderer.dispose();
    }
  });

  it("makes the cell kernel again after a device restore, and dispatches it", async () => {
    const engine = await countingRenderEngine();
    const made = vi.spyOn(engine, "createComputeAsync");
    const renderer = new PhotorealRenderer(engine, "test view");
    const camera = frameWith([]).camera;
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    engine.restore();
    await renderer.prepare(VIEWPORT, "high", "eye", camera);
    renderer.render(new RecordingView(), frameOfDiscs(smallDiscs()));
    expect([
      made.mock.calls.filter(([pair]) => pair === BODY_DISC_CELLS_KERNEL).length,
      engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS).length,
    ]).toEqual([2, 1]);
    renderer.dispose();
  });

  it("keeps three frames of a photorealistic primary and two instruments in the timer's resolves", async () => {
    // Each render resolves its own pass times; a dispatch's wait for the next render's.
    const engine = await countingRenderEngine();
    const views = [
      { name: "primary", viewport: { widthPx: 1920, heightPx: 1080 }, role: "eye" as const },
      { name: "instrument 1", viewport: { widthPx: 480, heightPx: 360 }, role: "camera" as const },
      { name: "instrument 2", viewport: { widthPx: 480, heightPx: 360 }, role: "camera" as const },
    ];
    const camera = frameWith([]).camera;
    const drawn = await Promise.all(
      views.map(async ({ name, viewport, role }) => {
        const renderer = new PhotorealRenderer(engine, name);
        await renderer.prepare(viewport, "high", role, camera);
        return { viewport, role, renderer, canvas: new RecordingView() };
      }),
    );
    const overlay = (): FrameSubmission => SYMBOLOGY;
    for (const view of drawn) {
      view.renderer.render(view.canvas, {
        ...frameOfDiscs(smallDiscs(view.viewport), view.viewport),
        role: view.role,
        meter: view.role === "eye" ? "average" : null,
        overlay,
      });
    }
    const resolves =
      engine.targetFrames.length + drawn.reduce((sum, view) => sum + view.canvas.frames.length, 0);
    expect(engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS)).toHaveLength(3);
    expect(3 * resolves).toBeLessThanOrEqual(TIMING_FRAMES_IN_FLIGHT);
    for (const view of drawn) {
      view.renderer.dispose();
    }
  });

  it("keeps three frames of three views in the timer's resolves with a dispatch a set of textures (R07.T8.e–f)", async () => {
    // A dispatch's times wait for the next render's resolve, so its sets add none.
    const engine = await countingRenderEngine();
    const views = [
      { name: "primary", viewport: { widthPx: 1920, heightPx: 1080 }, role: "eye" as const },
      { name: "instrument 1", viewport: { widthPx: 480, heightPx: 360 }, role: "camera" as const },
      { name: "instrument 2", viewport: { widthPx: 480, heightPx: 360 }, role: "camera" as const },
    ];
    const camera = frameWith([]).camera;
    const drawn = await Promise.all(
      views.map(async ({ name, viewport, role }) => {
        const renderer = new PhotorealRenderer(engine, name);
        await renderer.prepare(viewport, "high", role, camera);
        return { viewport, role, renderer, canvas: new RecordingView() };
      }),
    );
    const overlay = (): FrameSubmission => SYMBOLOGY;
    for (const view of drawn) {
      view.renderer.render(view.canvas, {
        ...frameOfDiscs(severalSets(view.viewport), view.viewport),
        role: view.role,
        meter: view.role === "eye" ? "average" : null,
        overlay,
      });
    }
    const resolves =
      engine.targetFrames.length + drawn.reduce((sum, view) => sum + view.canvas.frames.length, 0);
    expect(engine.dispatched.filter((d) => d.pass === DISC_CELLS_PASS)).toHaveLength(6);
    expect(3 * resolves).toBeLessThanOrEqual(TIMING_FRAMES_IN_FLIGHT);
    for (const view of drawn) {
      view.renderer.dispose();
    }
  });
});

/**
 * Bodies of two sets of textures on `viewport` (R07.T8.e–f): a 13 px disc and a 40 px one, whose
 * limb takes the pass, and a 13 px disc of a one-class map.
 */
function severalSets(viewport: Viewport = VIEWPORT): LitBodyInput[] {
  const third = viewport.widthPx / 4;
  const law = aLitBody().photometry.law;
  return [
    bodyAcross(13, third, "0200080020000000.0411", viewport),
    bodyAcross(40, 0, "0200080020000000.0412", viewport),
    {
      ...bodyAcross(13, -third, "0200080020000000.0413", viewport),
      surface: {
        kind: "class-map",
        weights: { kind: "texture", name: "test class map" },
        texels: packClassMap(1, 1, () => [1]),
        laws: [law],
        elsewhere: law,
        normals: null,
      },
      rotation: rotation3FromRows([vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)]),
    },
  ];
}
