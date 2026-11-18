import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { waterWorldAnswer, waterWorldHeader } from "../../test/oceanFixtures";
import { R05_EARTH_REFERENCE } from "../atmosphere/earth";
import { AtmosphereTables } from "../atmosphere/tables";
import type { LitBodyInput } from "../bodies/draw";
import { sphereFootprint } from "../bodies/regime";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import type { FrameSubmission, RenderView } from "../engine/types";
import { oceanSeaOf } from "../ocean/oceanBody";
import { oceanMapTextureSpec, packOceanTexels } from "../ocean/oceanMap";
import { oceanMaskOf } from "../ocean/oceanMask";
import { OCEAN_PASS_LABEL } from "../ocean/oceanPass";
import { BLOOM_THRESHOLD_EXPOSED } from "../post/bloom";
import { AU_M } from "../scenes/kept";
import { PHOTOREAL_PASS_LABELS } from "./passes";
import {
  type FrameAtmosphere,
  type FrameOcean,
  type PhotorealFrame,
  PhotorealRenderer,
} from "./renderer";

const VIEWPORT = { widthPx: 64, heightPx: 36 };

const EARTH = "0200080020000000.0300";

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

type Engine = Awaited<ReturnType<typeof countingRenderEngine>>;

/** A prepared renderer on the counting engine, with a view recording its canvas frames. */
async function preparedRenderer(): Promise<{
  readonly engine: Engine;
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

/** An Earth-like body at `centreM` from the camera, its body-fixed axes the galactic ones. */
function planetAt(centreM: ReturnType<typeof vec3>): LitBodyInput {
  const lit = aLitBody();
  return {
    id: EARTH,
    centreM,
    figure: lit.figure,
    photometry: lit.photometry,
    rotation: IDENTITY_ROTATION,
    lighting: undefined,
  };
}

/** The planet's sea: a water world under a 7 m/s wind, its map on `engine`. */
function seaOf(engine: Engine, planet: LitBodyInput): FrameOcean {
  const answer = waterWorldAnswer(4000, 64, 152);
  const mask = oceanMaskOf(answer);
  const header = {
    ...waterWorldHeader(EARTH),
    equatorial_radius_m: planet.figure.equatorialRadiusM,
    polar_radius_m: planet.figure.polarRadiusM,
  };
  const sea = oceanSeaOf(
    header,
    {
      answer,
      mask,
      texels: packOceanTexels(answer, mask, { from: 0, to: 0, weight: 0 }),
      texture: engine.createTexture(oceanMapTextureSpec(`ocean map ${EARTH}`, answer.level)),
    },
    [0.2, 0.18, 0.15],
  );
  if (sea === null) {
    throw new Error("the water world has a sea");
  }
  return {
    id: EARTH,
    sea,
    camera: {
      positionM: vec3(-planet.centreM.x, -planet.centreM.y, -planet.centreM.z),
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
      viewport: VIEWPORT,
    },
    toBodyFixed: IDENTITY_QUATERNION,
    time: { seconds: 1_000, nanos: 0 },
  };
}

/**
 * A frame of `planet` and its sea, lit by the Sun along +x, at `exposureScale`, a depth writer over
 * the planet, standing for R10's terrain, so that it is drawn as a mesh though it has no air.
 */
function frameOf(planet: LitBodyInput, ocean: FrameOcean, exposureScale = 1e-4): PhotorealFrame {
  const camera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
  const writer = sphereFootprint(planet.centreM, planet.figure.equatorialRadiusM, camera, VIEWPORT);
  return {
    camera,
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    role: "eye",
    setting: "high",
    exposureScale,
    sky: [],
    starSprites: [],
    hostDraws: new Map(),
    glareSources: [],
    lights: [{ disc: aHostDisc(), centreM: vec3(AU_M, 0, planet.centreM.z) }],
    bodies: [planet],
    depthWriters: writer === null ? [] : [writer],
    previousRegimes: new Map(),
    overlay: null,
    meter: "average",
    oceans: [ocean],
  };
}

/** The labels of the scene target's passes, in order. */
function labelsOf(engine: Engine): string[] {
  return engine.targetFrames.map((f) => f.label);
}

describe("the photorealistic renderer's ocean (R11.T8.b)", () => {
  it("draws a mesh body's sea after the bodies pass and before the discs and the air, one draw", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    const ocean = seaOf(engine, planet);
    const plan = renderer.render(view, frameOf(planet, ocean));
    expect(plan?.regimes.get(EARTH)).toBe("mesh");
    const labels = labelsOf(engine);
    const at = labels.indexOf(OCEAN_PASS_LABEL);
    expect(labels.slice(at - 1, at + 2)).toEqual([
      PHOTOREAL_PASS_LABELS.bodies,
      OCEAN_PASS_LABEL,
      PHOTOREAL_PASS_LABELS.discs,
    ]);
    const pass = engine.targetFrames[at];
    expect(pass?.colourLoad).toBe("load");
    expect(pass?.draws.map((draw) => draw.material.name)).toEqual(["ocean far pass"]);
    const draw = pass?.draws[0];
    expect(draw?.textures["oceanMap"]).toBe(ocean.sea.map.texture);
    // Without its air drawn, the sea is lit through the airless tables, made once.
    expect(draw?.textures["atmosphereBody"]?.name).toBe("test view ocean: no air");
    // Its camera from the body's centre, its figure on the datum as a mesh.
    expect(Array.from(draw?.uniforms["camera"] ?? [], (v) => v + 0)).toEqual([0, 0, 3e7, 1]);
    // The sea level below the datum, one source drawn.
    expect(Array.from(draw?.uniforms["datum"] ?? [])).toEqual(
      Array.from(new Float32Array([6.371e6, 6.371e6, -120, 1])),
    );
    renderer.dispose();
  });

  it("caps the glint at the bloom threshold in the pre-exposed units, and pre-exposes the light", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    renderer.render(view, frameOf(planet, seaOf(engine, planet), 1e-4));
    const draw = engine.targetFrames.find((f) => f.label === OCEAN_PASS_LABEL)?.draws[0];
    expect(draw?.uniforms["farSun"]?.[3]).toBeCloseTo(BLOOM_THRESHOLD_EXPOSED, 4);
    const light = draw?.uniforms["sourceLight0"]?.[1] ?? 0;
    // The Sun's ~1.3 × 10⁵ lx at 1 au, times the pre-exposure.
    expect(light).toBeGreaterThan(10);
    expect(light).toBeLessThan(15);
    renderer.dispose();
  });

  it("lights a sea through its body's air's tables where the frame draws them", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    const ocean = seaOf(engine, planet);
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6.371e6);
    const air: FrameAtmosphere = {
      id: EARTH,
      tables,
      camera: ocean.camera,
      toBodyFixed: IDENTITY_QUATERNION,
    };
    renderer.render(view, { ...frameOf(planet, ocean), atmospheres: [air] });
    const labels = labelsOf(engine);
    expect(labels.indexOf(OCEAN_PASS_LABEL)).toBeLessThan(labels.indexOf("atmosphere composite"));
    const draw = engine.targetFrames.find((f) => f.label === OCEAN_PASS_LABEL)?.draws[0];
    expect(draw?.textures["atmosphereBody"]).toBe(tables.oblateBody);
    renderer.dispose();
    tables.dispose();
  });

  it("draws no sea for a body drawn as a disc or a point, nor for a frame of none", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const far = planetAt(vec3(0, 0, -3e10));
    const plan = renderer.render(view, frameOf(far, seaOf(engine, far)));
    expect(plan?.regimes.get(EARTH)).not.toBe("mesh");
    const near = planetAt(vec3(0, 0, -3e7));
    renderer.render(view, { ...frameOf(near, seaOf(engine, near)), oceans: [] });
    expect(labelsOf(engine)).not.toContain(OCEAN_PASS_LABEL);
    renderer.dispose();
  });

  it("adds the glint's light past the threshold as a glare source, which the tone-mapping pass counts", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    // At an exposure under which the glint's peak passes the threshold.
    renderer.render(view, frameOf(planet, seaOf(engine, planet), 1e-2));
    const glare = renderer.glareSources;
    expect(glare).toHaveLength(1);
    const [source] = glare;
    // Between the camera's nadir (−z) and the Sun (+x), on the camera's side of the planet.
    expect(source?.direction.x).toBeGreaterThan(0);
    expect(source?.direction.z).toBeLessThan(0);
    expect(source?.excessLuminance[1]).toBeGreaterThan(0);
    expect(view.frames.at(-1)?.draws[0]?.uniforms["sourceCount"]?.[0]).toBe(1);
    renderer.dispose();
  });

  it("holds the glare source while its camera and sun stand, and forms it again once they move", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    const ocean = seaOf(engine, planet);
    renderer.render(view, frameOf(planet, ocean, 1e-2));
    const first = renderer.glareSources[0]?.luminance;
    renderer.render(view, frameOf(planet, ocean, 1e-2));
    // 10 km of 30,000: inside GLARE_HOLD's 10⁻³, the source held, its direction afresh.
    const nudged = planetAt(vec3(0, 1e4, -3e7));
    renderer.render(
      view,
      frameOf(nudged, { ...ocean, camera: seaOf(engine, nudged).camera }, 1e-2),
    );
    const held = renderer.glareSources[0]?.luminance;
    const moved = planetAt(vec3(0, 3e5, -3e7));
    renderer.render(view, frameOf(moved, { ...ocean, camera: seaOf(engine, moved).camera }, 1e-2));
    expect(first).toBeDefined();
    expect(held).toBe(first);
    expect(renderer.glareSources[0]?.luminance).not.toBe(first);
    renderer.dispose();
  });

  it("holds the glare source of a sea whose air the frame draws, frame after frame", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    const ocean = seaOf(engine, planet);
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6.371e6);
    const air: FrameAtmosphere = {
      id: EARTH,
      tables,
      camera: ocean.camera,
      toBodyFixed: IDENTITY_QUATERNION,
    };
    renderer.render(view, { ...frameOf(planet, ocean, 1e-2), atmospheres: [air] });
    const first = renderer.glareSources.at(-1)?.luminance;
    renderer.render(view, { ...frameOf(planet, ocean, 1e-2), atmospheres: [air] });
    expect(first).toBeDefined();
    expect(renderer.glareSources.at(-1)?.luminance).toBe(first);
    renderer.dispose();
    tables.dispose();
  });

  it("dims the sea's light by its record's eclipses, one buffer of rows a sea under ocean", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = planetAt(vec3(0, 0, -3e7));
    const plan = renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    expect(plan?.discs.some((record) => record.body === EARTH)).toBe(true);
    const draw = engine.targetFrames.find((f) => f.label === OCEAN_PASS_LABEL)?.draws[0];
    const rows = draw?.storageBuffers?.["oceanEclipse"];
    expect(rows?.name).toBe(`test view ocean eclipse ${EARTH}`);
    // The record's one star, at its slot 0: the Sun's light from the record, at 1 au.
    const written = engine.writes.findLast((write) => write.buffer === rows?.name);
    const header =
      written === undefined
        ? new Float32Array(4)
        : new Float32Array(written.data.slice(0, 16).buffer);
    expect(header[0]).toBe(1);
    expect(header[3]).toBeCloseTo(6.371e6, -1);
    renderer.dispose();
  });

  it("releases its sea's material and its airless tables with the view", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const materials = vi.spyOn(engine, "releaseMaterial");
    const textures = vi.spyOn(engine, "releaseTexture");
    const buffers = vi.spyOn(engine, "releaseBuffer");
    const planet = planetAt(vec3(0, 0, -3e7));
    renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    renderer.dispose();
    expect(
      buffers.mock.calls.filter(([buffer]) => buffer.name === `test view ocean eclipse ${EARTH}`)
        .length,
    ).toBe(1);
    expect(
      materials.mock.calls.filter(([material]) => material.name === "ocean far pass").length,
    ).toBe(1);
    expect(
      textures.mock.calls.filter(([texture]) => texture.name === "test view ocean: no air").length,
    ).toBe(1);
  });
});

/** The names of the handles a spy's calls released. */
function names(calls: ReadonlyArray<ReadonlyArray<{ readonly name: string }>>): string[] {
  return calls.map(([handle]) => handle?.name ?? "");
}

describe("the photorealistic renderer's near sea (R11.T8.c)", () => {
  /** The planet with its camera `heightM` above its sea level, looking down its −z. */
  function lowOver(heightM: number): LitBodyInput {
    // The sea level 120 m below the datum (`waterWorldHeader`).
    return planetAt(vec3(0, 0, -(6.371e6 - 120 + heightM)));
  }

  it("copies the scene's depth and colour first, then draws the grid beside the far draw's basins alone", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = lowOver(600);
    const ocean = seaOf(engine, planet);
    renderer.render(view, frameOf(planet, ocean));
    const labels = labelsOf(engine);
    const at = labels.indexOf(OCEAN_PASS_LABEL);
    // The high setting refracts: its depth's bits and its colour, then the pass.
    expect(labels.slice(at - 2, at + 1)).toEqual([
      "ocean copies",
      "ocean copies",
      OCEAN_PASS_LABEL,
    ]);
    const pass = engine.targetFrames[at];
    expect(pass?.draws.map((draw) => draw.material.name)).toEqual([
      "ocean far pass",
      "ocean near pass",
    ]);
    const [far, near] = pass?.draws ?? [];
    expect(far?.uniforms["farBed"]?.[3]).toBe(1);
    expect(near?.mesh.name).toBe("test view ocean grid");
    expect(near?.textures["oceanSceneDepth"]?.name).toBe("test view:ocean depth copy colour");
    expect(near?.textures["oceanSceneColour"]?.name).toBe("test view:ocean colour copy colour");
    expect(near?.storageBuffers?.["oceanWaves"]?.name).toBe(`test view ocean waves ${EARTH}`);
    // The camera's height over the sea, refraction on.
    expect(near?.uniforms["nearUpwind"]?.[3]).toBeCloseTo(600, 0);
    expect(near?.uniforms["camera"]?.[3]).toBe(1);
    const copy = engine.targetFrames[at - 1];
    expect(copy?.colourFrom).toBeDefined();
    renderer.dispose();
  });

  it("binds the colour's stand-in on the low setting, which copies no colour and refracts nothing", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = lowOver(600);
    renderer.render(view, { ...frameOf(planet, seaOf(engine, planet)), setting: "low" });
    const near = engine.targetFrames.find((f) => f.label === OCEAN_PASS_LABEL)?.draws[1];
    expect(near?.textures["oceanSceneColour"]?.name).toBe("test view ocean: no colour");
    expect(near?.uniforms["camera"]?.[3]).toBe(0);
    expect(labelsOf(engine).filter((label) => label === "ocean copies")).toHaveLength(1);
    renderer.dispose();
  });

  it("draws no near sea from 20 km up: the far draw alone, no copy", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const high = lowOver(25_000);
    renderer.render(view, frameOf(high, seaOf(engine, high)));
    expect(labelsOf(engine)).not.toContain("ocean copies");
    expect(
      engine.targetFrames
        .find((f) => f.label === OCEAN_PASS_LABEL)
        ?.draws.map((draw) => draw.material.name),
    ).toEqual(["ocean far pass"]);
    renderer.dispose();
  });

  it("lets its grid and its waves go at a frame with no near sea", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const released: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        released.push(event.name);
      }
    });
    const low = lowOver(600);
    renderer.render(view, frameOf(low, seaOf(engine, low)));
    const high = lowOver(25_000);
    renderer.render(view, frameOf(high, seaOf(engine, high)));
    expect(released).toEqual(
      expect.arrayContaining([`test view ocean waves ${EARTH}`, "test view ocean grid:position"]),
    );
    renderer.dispose();
  });

  it("makes its copies again at the next near frame after one with none", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const made = (name: string): number =>
      engine.textureSpecs.filter((spec) => spec.name === name).length;
    const low = lowOver(600);
    const high = lowOver(25_000);
    renderer.render(view, frameOf(low, seaOf(engine, low)));
    renderer.render(view, frameOf(high, seaOf(engine, high)));
    renderer.render(view, frameOf(low, seaOf(engine, low)));
    expect([
      made("test view:ocean depth copy colour"),
      made("test view:ocean colour copy colour"),
    ]).toEqual([2, 2]);
    renderer.dispose();
  });

  it("lets the colour's copy go when the setting stops refracting, the depth's kept", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const planet = lowOver(600);
    renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    const copy = engine.targetFrames.find(
      (f) => f.colourFrom !== undefined && f.label === "ocean copies",
    );
    expect(copy).toBeDefined();
    const before = engine.targetFrames.length;
    renderer.render(view, { ...frameOf(planet, seaOf(engine, planet)), setting: "low" });
    const later = engine.targetFrames.slice(before);
    expect(later.filter((f) => f.label === "ocean copies")).toHaveLength(1);
    expect(later.some((f) => f.colourFrom !== undefined && f.label === "ocean copies")).toBe(false);
    renderer.dispose();
  });

  it("makes its grid again at a new size", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const meshes = vi.spyOn(engine, "createMesh");
    const planet = lowOver(600);
    renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    const wider = { widthPx: 96, heightPx: 36 };
    renderer.render(view, {
      ...frameOf(planet, seaOf(engine, planet)),
      viewport: wider,
      presentedPx: wider,
    });
    const grids = meshes.mock.calls.filter(([spec]) => spec.name === "test view ocean grid");
    expect(grids.map(([spec]) => spec.positions.length)).toHaveLength(2);
    expect(grids[1]?.[0].positions.length).toBeGreaterThan(grids[0]?.[0].positions.length ?? 0);
    renderer.dispose();
  });

  it("forgets its waves' rows at a device restore, and makes them on the restored device", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const made: string[] = [];
    engine.onAllocation((event) => {
      if (event.kind === "created") {
        made.push(event.name);
      }
    });
    const waves = (): number =>
      made.filter((name) => name === `test view ocean waves ${EARTH}`).length;
    const planet = lowOver(600);
    renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    const first = waves();
    engine.restore();
    await renderer.prepare(VIEWPORT, "high", "eye", {
      orientation: IDENTITY_QUATERNION,
      fovXRad: Math.PI / 3,
    });
    renderer.render(view, frameOf(planet, seaOf(engine, planet)));
    expect([first, waves()]).toEqual([1, 2]);
    renderer.dispose();
  });

  it("releases its near sea's materials, copies, grid, waves and colour stand-in with the view", async () => {
    const { engine, view, renderer } = await preparedRenderer();
    const materials = vi.spyOn(engine, "releaseMaterial");
    const meshes = vi.spyOn(engine, "releaseMesh");
    const buffers = vi.spyOn(engine, "releaseBuffer");
    const textures = vi.spyOn(engine, "releaseTexture");
    const planet = lowOver(600);
    renderer.render(view, { ...frameOf(planet, seaOf(engine, planet)), setting: "low" });
    renderer.dispose();
    expect(names(materials.mock.calls)).toEqual(
      expect.arrayContaining(["ocean near pass", "ocean depth copy"]),
    );
    expect(names(meshes.mock.calls)).toContain("test view ocean grid");
    expect(names(buffers.mock.calls)).toContain(`test view ocean waves ${EARTH}`);
    expect(names(textures.mock.calls)).toContain("test view ocean: no colour");
  });
});
