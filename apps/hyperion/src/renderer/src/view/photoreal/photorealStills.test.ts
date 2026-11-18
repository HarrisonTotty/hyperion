import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { R05_EARTH_REFERENCE } from "../atmosphere/earth";
import { ATMOSPHERE_COMPOSITE_PASS } from "../atmosphere/hillaire";
import { AtmosphereTables } from "../atmosphere/tables";
import { countingRenderEngine, type CountingRenderEngine } from "../../test/countingRenderEngine";
import {
  type FakeStillTerrainSession,
  fakeStillTerrainSession,
  stillTerrainCapture,
} from "../../test/fakeStillTerrain";
import { aHostDisc, aLitBody } from "../../test/litFixtures";
import { TEST_STILL_REQUEST } from "../../test/stillFixtures";
import { type BodyFramePlan, planLitBodies } from "../bodies/draw";
import { NEAR_PLANE_M, perspectiveReversedInfinite } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { sphereFootprint } from "../bodies/regime";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { oceanSeaOf } from "../ocean/oceanBody";
import { oceanMapTextureSpec, packOceanTexels } from "../ocean/oceanMap";
import { oceanMaskOf } from "../ocean/oceanMask";
import { GLARE_SOURCE_BYTES } from "../post/bloomChain";
import { waterWorldAnswer, waterWorldHeader } from "../../test/oceanFixtures";
import type { FrameSubmission, PassTimes, TexelRect, ViewSize } from "../engine/types";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import { AU_M } from "../scenes/kept";
import type { MeasuredPass } from "../stills/batchController";
import { formatNumber } from "../../lib/format";
import { runStillJob, type StillRenderer, type StillScene } from "../stills/stillJob";
import { STILL_MAX_SIDE_PX, STILL_TERRAIN_STREAMING } from "../stills/stillLabel";
import { STILL_TERRAIN_VIEW_ID } from "../stills/stillTerrain";
import { STILL_TILE_ADD_MATERIAL, STILL_TILE_FIRST_MATERIAL } from "../stills/stillAccumulate";
import { planTiles, tileProjection } from "../stills/stillPlan";
import { TONEMAP_MATERIAL } from "../post/tonemap";
import { timedPass } from "../../test/passTimes";
import type { QualitySetting } from "../quality/qualitySetting";
import {
  type DrawnPhotorealFrame,
  type FrameTerrain,
  type PhotorealFrame,
  type PhotorealStillView,
  PhotorealRenderer,
  PhotorealStillRenderer,
  type StillFrameAsk,
  PhotorealStills,
  STILL_SAMPLES,
  STILL_UNAVAILABLE,
  stillJitterPx,
} from "./renderer";

/** The still's size, px: a width no multiple of 64, as a view's need not be. */
const SIZE: ViewSize = { widthPx: 100, heightPx: 70 };

const FOV_X_RAD = Math.PI / 3;

/** The frozen frame of a view of one lit body and its star, at {@link SIZE}. */
function viewFrame(size: ViewSize = SIZE): PhotorealFrame {
  const lit = aLitBody();
  return {
    camera: { orientation: IDENTITY_QUATERNION, fovXRad: FOV_X_RAD },
    viewport: size,
    presentedPx: size,
    role: "eye",
    setting: "high",
    exposureScale: 1e-4,
    sky: [],
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

/** The plan of `frame`, as the view's renderer made it, with `regimes` in its own's place. */
function planOf(frame: PhotorealFrame, regimes?: BodyFramePlan["regimes"]): BodyFramePlan {
  const plan = planLitBodies(
    frame.bodies,
    frame.lights,
    {
      camera: frame.camera,
      viewport: frame.viewport,
      presentedPx: frame.presentedPx,
      exposureScale: frame.exposureScale,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: frame.setting,
    },
    new Map(),
  );
  return regimes === undefined ? plan : { ...plan, regimes };
}

/** A view's last frame drawn: `frame` and its plan. */
function drawnOf(frame: PhotorealFrame): DrawnPhotorealFrame {
  return { frame, plan: planOf(frame) };
}

/** A frame's terrain that draws nothing: a body handed over to it all the same. */
const NO_TERRAIN: FrameTerrain = {
  surveyCoverage: () => null,
  surveyedWhole: () => false,
  draws: () => [],
  lateDraws: () => [],
  symbologyDepth: () => [],
  sceneDepth: () => false,
  visibility: () => null,
  opaqueDraws: () => [],
};

/** A view attached to the hook: its drawn frame the test sets. */
function viewOver(
  engine: CountingRenderEngine,
  drawn: () => DrawnPhotorealFrame | null,
): PhotorealStillView {
  const { size: _size, ...request } = TEST_STILL_REQUEST;
  return {
    engine,
    timer: () => "absent",
    frameIntervalNs: () => 16_683_350,
    setting: () => "high",
    drawn,
    request: () => request,
  };
}

/** A view attached to the hook that gives `session` for a still's terrain. */
function viewWithTerrain(
  engine: CountingRenderEngine,
  drawn: () => DrawnPhotorealFrame | null,
  session: FakeStillTerrainSession,
): PhotorealStillView {
  return { ...viewOver(engine, drawn), terrain: () => stillTerrainCapture(session) };
}

/** The view's frame with its one body handed over to a terrain that draws nothing. */
function handedOver(): DrawnPhotorealFrame {
  const frame = { ...viewFrame(), terrain: NO_TERRAIN };
  return { frame, plan: planOf(frame, new Map([[aLitBody().body, "terrain"]])) };
}

/** The engine's target frames into the still's targets, by label. */
function stillPasses(engine: CountingRenderEngine): ReadonlyArray<FrameSubmission> {
  return engine.targetFrames.filter((frame) => frame.label.startsWith("still"));
}

/** Whether `value` is a texel rect, as `readTexture`'s third argument. */
function isTexelRect(value: unknown): value is TexelRect {
  return (
    typeof value === "object" &&
    value !== null &&
    "width" in value &&
    typeof value.width === "number" &&
    "height" in value &&
    typeof value.height === "number"
  );
}

/** The scene `stills` freezes now, which must give one. */
function sceneOf(stills: PhotorealStills): StillScene {
  const snapshot = stills.snapshot();
  if (snapshot.kind === "unavailable") {
    throw new Error(`no still: ${snapshot.cause}`);
  }
  return snapshot.scene;
}

/** Every unit of `renderer`, each its own submission. */
function submitAll(renderer: StillRenderer): void {
  for (let unit = 0; unit < renderer.units.length; unit += 1) {
    renderer.submit(unit, 1);
  }
}

describe("the photorealistic still source (R11.T10.b)", () => {
  it("gives no still while no view is attached, nor while its last frame is its wireframe", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    expect([stills.unavailable(), stills.snapshot()]).toEqual([
      STILL_UNAVAILABLE.noView,
      { kind: "unavailable", cause: STILL_UNAVAILABLE.noView },
    ]);
    stills.attach(viewOver(engine, () => null));
    expect([stills.unavailable(), stills.snapshot()]).toEqual([
      STILL_UNAVAILABLE.noImage,
      { kind: "unavailable", cause: STILL_UNAVAILABLE.noImage },
    ]);
  });

  it("gives no still while the view hands a body over to its terrain, which a still does not draw", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    const frame = { ...viewFrame(), terrain: NO_TERRAIN };
    const plan = planOf(frame, new Map([[aLitBody().body, "terrain"]]));
    stills.attach(viewOver(engine, () => ({ frame, plan })));
    expect(stills.snapshot()).toEqual({ kind: "unavailable", cause: STILL_UNAVAILABLE.terrain });
  });

  it("gives a still of a view handing a body over where it gives its terrain, the still's own view noted at bulk priority", async () => {
    const engine = await countingRenderEngine();
    const session = fakeStillTerrainSession();
    const stills = new PhotorealStills();
    stills.attach(viewWithTerrain(engine, handedOver, session));
    const unavailable = stills.unavailable();
    const renderer = stills.renderer(sceneOf(stills));
    const [noted] = session.noted;
    expect([
      unavailable,
      session.holds,
      noted?.[0],
      noted?.[1].streamPriority,
      noted?.[1].selectionPx,
    ]).toEqual([null, [[aLitBody().body]], STILL_TERRAIN_VIEW_ID, "bulk", SIZE]);
    renderer.dispose();
    expect([session.forgotten, session.releases.count]).toEqual([[STILL_TERRAIN_VIEW_ID], 1]);
  });

  it("has a still with terrain streamed as its own terrain view has, and one with none always", async () => {
    const engine = await countingRenderEngine();
    const session = fakeStillTerrainSession();
    const stills = new PhotorealStills();
    stills.attach(viewWithTerrain(engine, handedOver, session));
    const renderer = stills.renderer(sceneOf(stills));
    const before = renderer.streamed();
    session.streamed = true;
    const after = renderer.streamed();
    renderer.dispose();
    const plain = new PhotorealStills();
    plain.attach(viewWithTerrain(engine, () => drawnOf(viewFrame()), fakeStillTerrainSession()));
    const without = plain.renderer(sceneOf(plain));
    expect([before, after, without.streamed()]).toEqual([false, true, true]);
    without.dispose();
  });

  it("labels a still TERRAIN: STREAMING whose frustum has not streamed within the timeout, and one that has not", async () => {
    const engine = await countingRenderEngine();
    vi.spyOn(engine, "readTexture").mockImplementation((...args: unknown[]) => {
      const rect = args[2];
      return Promise.resolve(new ArrayBuffer(isTexelRect(rect) ? rect.width * rect.height * 4 : 0));
    });
    const labels = async (streamed: boolean): Promise<ReadonlyArray<string>> => {
      const session = fakeStillTerrainSession();
      session.streamed = streamed;
      let nowNs = 0;
      const stills = new PhotorealStills({
        samples: 1,
        // A second an animation frame: the 20 s wait ends at its twentieth.
        nextFrame: () => {
          nowNs += 1e9;
          return Promise.resolve();
        },
        nowNs: () => nowNs,
      });
      stills.attach(viewWithTerrain(engine, handedOver, session));
      const scene = sceneOf(stills);
      const outcome = await runStillJob({
        device: stills.device,
        renderer: stills.renderer(scene),
        request: scene.request,
        setting: stills.setting,
        frameIntervalNs: stills.frameIntervalNs(),
        encode: () => Promise.resolve(new Uint8Array([1])),
        save: (_png, label) =>
          Promise.resolve({ kind: "saved", still: { id: "20261118T000000000Z-00000000", label } }),
        onStep: () => undefined,
      });
      return outcome.kind === "saved" ? outcome.still.label.annunciations : ["not saved"];
    };
    const held = TEST_STILL_REQUEST.annunciations;
    expect([await labels(false), await labels(true)]).toEqual([
      [...held, STILL_TERRAIN_STREAMING],
      held,
    ]);
  });

  it("takes a still at a multiple of the view's output size, its frame made again there (R11.T10.e.1)", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    const presented = { widthPx: 160, heightPx: 112 };
    const asked: StillFrameAsk[] = [];
    const drawn: DrawnPhotorealFrame = {
      ...drawnOf({ ...viewFrame(), presentedPx: presented }),
      remake: (ask) => {
        asked.push(ask);
        return viewFrame(ask.viewport);
      },
    };
    stills.attach(viewOver(engine, () => drawn));
    const output = stills.outputSize();
    const snapshot = stills.snapshot({ scale: 3 });
    const scene = snapshot.kind === "scene" ? snapshot.scene : null;
    const renderer = scene === null ? null : stills.renderer(scene);
    const still = { widthPx: 480, heightPx: 336 };
    expect([
      output,
      scene?.request.size,
      renderer?.tiling.size,
      asked.map((ask) => [ask.viewport, ask.setting, ask.terrain]),
    ]).toEqual([presented, still, still, [[still, "high", undefined]]]);
    renderer?.dispose();
  });

  it("gives no still past the largest still's side at the multiple asked (R11.T10.e.1)", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    const drawn: DrawnPhotorealFrame = {
      ...drawnOf({ ...viewFrame(), presentedPx: { widthPx: 3000, heightPx: 2000 } }),
      remake: (ask) => viewFrame(ask.viewport),
    };
    stills.attach(viewOver(engine, () => drawn));
    expect([stills.unavailable(), stills.snapshot({ scale: 3 })]).toEqual([
      null,
      { kind: "unavailable", cause: STILL_UNAVAILABLE.oversized },
    ]);
  });

  it("gives no still of a frame held back at a cut, drawn at the exposure from before it", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    stills.attach(viewOver(engine, () => drawnOf({ ...viewFrame(), present: false })));
    expect(stills.unavailable()).toBe(STILL_UNAVAILABLE.settling);
  });

  it("states the largest still's side in its cause, as the main process bounds it", () => {
    expect(STILL_UNAVAILABLE.oversized).toContain(formatNumber(STILL_MAX_SIDE_PX, 0));
  });

  it("freezes the view's last photorealistic frame and its label's inputs at the request, at the frame's own size", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    let drawn: DrawnPhotorealFrame = drawnOf(viewFrame());
    stills.attach(viewOver(engine, () => drawn));
    const scene = sceneOf(stills);
    // A frame the view draws after the request is not the still's.
    drawn = drawnOf(viewFrame({ widthPx: 320, heightPx: 200 }));
    const renderer = stills.renderer(scene);
    expect([scene.request.size, scene.request.readings, renderer.tiling.size]).toEqual([
      SIZE,
      TEST_STILL_REQUEST.readings,
      SIZE,
    ]);
    renderer.dispose();
  });

  it("refuses a scene it did not freeze", async () => {
    const stills = new PhotorealStills();
    const foreign: StillScene = { request: TEST_STILL_REQUEST };
    expect(() => stills.renderer(foreign)).toThrow(/not the still source's snapshot/u);
  });

  it("detaches only the view it attached, leaving a later one attached", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    const drawn: DrawnPhotorealFrame = drawnOf(viewFrame());
    const detachFirst = stills.attach(viewOver(engine, () => null));
    stills.attach(viewOver(engine, () => drawn));
    detachFirst();
    expect(stills.unavailable()).toBeNull();
  });

  it("hands the job a loss at the engine's device loss and GPU-process crash, and at no other fault", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    stills.attach(viewOver(engine, () => null));
    const losses = vi.fn<() => void>();
    stills.device.onLoss(losses);
    engine.inner.raiseFault({ kind: "shader-refused", effectName: "IMAGE" });
    engine.inner.raiseFault({ kind: "gpu-process-gone", count: 1 });
    engine.inner.loseDevice();
    expect(losses).toHaveBeenCalledTimes(2);
  });

  it("stops handing the job losses once the job removes its listener", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    stills.attach(viewOver(engine, () => null));
    const losses = vi.fn<() => void>();
    stills.device.onLoss(losses)();
    engine.inner.loseDevice();
    expect(losses).not.toHaveBeenCalled();
  });

  it("takes one sample of a scene that draws no figure, which the jitter would not move", async () => {
    const engine = await countingRenderEngine();
    const stills = new PhotorealStills();
    const far = viewFrame();
    stills.attach(viewOver(engine, () => drawnOf(far)));
    const renderer = stills.renderer(sceneOf(stills));
    const figures = planOf(far).meshes.length;
    expect([figures, renderer.units]).toEqual([0, ["scene 0", "bloom 0", "tonemap 0"]]);
    renderer.dispose();
  });

  it("takes eight samples on both settings, averaged in linear HDR (R11.T10.e.3; ruling 5)", () => {
    expect(STILL_SAMPLES).toEqual({ high: 8, low: 8 });
  });

  it("jitters each sample after the first by a Halton (2, 3) offset within half a pixel", () => {
    const offsets = Array.from({ length: 8 }, (_, k) => stillJitterPx(k));
    expect(offsets.slice(0, 3)).toEqual([
      [0, 0],
      [0, 1 / 3 - 0.5],
      [0.25 - 0.5, 2 / 3 - 0.5],
    ]);
    expect(offsets.every(([dx, dy]) => Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5)).toBe(true);
  });
});

describe("the photorealistic still renderer (R11.T10.b)", () => {
  /** A renderer of the view's frame, prepared, its unit times handed to `onUnitTimes`. */
  async function prepared(
    engine: CountingRenderEngine,
    samples = 3,
    onUnitTimes: (passes: ReadonlyArray<MeasuredPass>) => void = () => undefined,
  ): Promise<PhotorealStillRenderer> {
    const renderer = new PhotorealStillRenderer(engine, viewFrame(), {
      tileLimitPx: 64,
      samples,
      onUnitTimes,
      onDispose: () => undefined,
    });
    await renderer.prepare();
    return renderer;
  }

  it("makes its accumulation and its rgba8 tone mapping asynchronously, and no pipeline in its batches", async () => {
    const engine = await countingRenderEngine();
    const renderer = await prepared(engine);
    const made = { ...engine.counts };
    submitAll(renderer);
    expect([
      [STILL_TILE_FIRST_MATERIAL, STILL_TILE_ADD_MATERIAL].every((spec) =>
        engine.compiles.some(
          ({ name, targets }) => name === spec.name && targets[0] === "rgba16float",
        ),
      ),
      engine.compiles.some(
        ({ name, targets }) => name === TONEMAP_MATERIAL.name && targets[0] === "rgba8unorm",
      ),
      engine.counts.materials - made.materials,
      engine.counts.kernels - made.kernels,
    ]).toEqual([true, true, 0, 0]);
    renderer.dispose();
  });

  it("tone-maps a sea's glint past the threshold as a glare source, its buffer sized for it (R11.T8.b)", async () => {
    const engine = await countingRenderEngine();
    const lit = aLitBody();
    const centreM = vec3(0, 0, -3e7);
    const camera = { orientation: IDENTITY_QUATERNION, fovXRad: FOV_X_RAD };
    // A depth writer over the planet, standing for terrain, so that it is drawn as a mesh.
    const writer = sphereFootprint(centreM, lit.figure.equatorialRadiusM, camera, SIZE);
    const answer = waterWorldAnswer(4000, 64, 152);
    const mask = oceanMaskOf(answer);
    const sea = oceanSeaOf(
      {
        ...waterWorldHeader(lit.body),
        equatorial_radius_m: lit.figure.equatorialRadiusM,
        polar_radius_m: lit.figure.polarRadiusM,
      },
      {
        answer,
        mask,
        texels: packOceanTexels(answer, mask, { from: 0, to: 0, weight: 0 }),
        texture: engine.createTexture(oceanMapTextureSpec(`ocean map ${lit.body}`, answer.level)),
      },
      null,
    );
    if (sea === null) {
      throw new Error("the water world has a sea");
    }
    const base = viewFrame();
    const [planet] = base.bodies;
    if (planet === undefined) {
      throw new Error("the view's frame has its body");
    }
    const frame: PhotorealFrame = {
      ...base,
      exposureScale: 1e-2,
      lights: [{ disc: aHostDisc(), centreM: vec3(AU_M, 0, centreM.z) }],
      bodies: [{ ...planet, centreM, rotation: IDENTITY_ROTATION }],
      depthWriters: writer === null ? [] : [writer],
      oceans: [
        {
          id: lit.body,
          sea,
          camera: {
            positionM: vec3(0, 0, 3e7),
            orientation: IDENTITY_QUATERNION,
            fovXRad: FOV_X_RAD,
            viewport: SIZE,
          },
          toBodyFixed: IDENTITY_QUATERNION,
          time: { seconds: 0, nanos: 0 },
        },
      ],
    };
    const made: Array<{ readonly name: string; readonly bytes: number }> = [];
    engine.onAllocation((event) => {
      if (event.kind === "created") {
        made.push({ name: event.name, bytes: event.bytes });
      }
    });
    const renderer = new PhotorealStillRenderer(engine, frame, {
      tileLimitPx: 64,
      samples: 1,
      onUnitTimes: () => undefined,
      onDispose: () => undefined,
    });
    await renderer.prepare();
    submitAll(renderer);
    const tone = stillPasses(engine).find((each) => each.label === "still tonemap");
    expect(tone?.draws[0]?.uniforms["sourceCount"]?.[0]).toBe(1);
    // The still's own buffer, beside its scene renderer's of eight: the frame's none and its sea's one.
    expect(
      made.filter((each) => each.name === "still glare sources").map((each) => each.bytes),
    ).toEqual([8 * GLARE_SOURCE_BYTES, GLARE_SOURCE_BYTES]);
    renderer.dispose();
  });

  it("orders its units as each sample's tiles, sample by sample, then its bloom, then its tone mapping (R11.T10.e.2)", async () => {
    const engine = await countingRenderEngine();
    const renderer = await prepared(engine);
    const tiles = ["scene 0", "scene 1", "scene 2", "scene 3"];
    expect(renderer.units).toEqual([...tiles, ...tiles, ...tiles, "bloom 0", "tonemap 0"]);
    renderer.dispose();
  });

  it("draws each tile through its M · P from its origin, the first sample's unmoved and the later ones jittered (R11.T10.e.2)", async () => {
    const engine = await countingRenderEngine();
    const renderer = await prepared(engine);
    const before = engine.targetFrames.length;
    submitAll(renderer);
    const skies = engine.targetFrames.slice(before).filter((frame) => frame.label === "sky");
    const tiling = planTiles(SIZE, 64, "high");
    const drawn = { widthPx: 64, heightPx: 35 };
    const projectionOf = (x0Px: number, y0Px: number): number[] =>
      Array.from(
        tileProjection(FOV_X_RAD, SIZE, {
          column: 0,
          row: 0,
          rect: { x0Px, y0Px, x1Px: x0Px + 64, y1Px: y0Px + 35 },
          texture: drawn,
          bytesPerRow: 256,
        }),
      );
    const [dx, dy] = stillJitterPx(1);
    const last = tiling.tiles[3];
    expect([
      skies.length,
      skies.slice(0, 4).map((frame) => [frame.tileOriginPx, Array.from(frame.projection)]),
      [skies[7]?.tileOriginPx, Array.from(skies[7]?.projection ?? [])],
    ]).toEqual([
      12,
      tiling.tiles.map(({ rect }) => [[rect.x0Px, rect.y0Px], projectionOf(rect.x0Px, rect.y0Px)]),
      [
        [(last?.rect.x0Px ?? 0) + dx, (last?.rect.y0Px ?? 0) + dy],
        projectionOf((last?.rect.x0Px ?? 0) + dx, (last?.rect.y0Px ?? 0) + dy),
      ],
    ]);
    renderer.dispose();
  });

  it("adds each tile's rect to the assembled image at 1 ÷ n, the first of each tile with its meter class (R11.T10.e.2)", async () => {
    const engine = await countingRenderEngine();
    const renderer = await prepared(engine);
    submitAll(renderer);
    const accumulations = stillPasses(engine).filter(
      (frame) => frame.label === "still accumulation",
    );
    const tiling = planTiles(SIZE, 64, "high");
    const rects = tiling.tiles.map(({ rect }) => ({
      x: rect.x0Px,
      y: rect.y0Px,
      width: rect.x1Px - rect.x0Px,
      height: rect.y1Px - rect.y0Px,
    }));
    expect(
      accumulations.map((frame) => [
        frame.scissor,
        frame.colourLoad,
        frame.draws.map((draw) => [
          draw.uniforms["weight"]?.[0],
          draw.uniforms["first"]?.[0],
          Array.from(draw.uniforms["origin"] ?? []),
        ]),
      ]),
    ).toEqual(
      [0, 1, 2].flatMap((k) =>
        rects.map((rect) => [
          rect,
          "load",
          [[Math.fround(1 / 3), k === 0 ? 1 : 0, [rect.x, rect.y, 0, 0]]],
        ]),
      ),
    );
    renderer.dispose();
  });

  it("draws a low view's still at the high setting's passes, its air's every kernel made before its first batch and none in a batch (R11.T10.e.3)", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6.371e6);
    const airAt = (setting: QualitySetting): PhotorealFrame => ({
      ...viewFrame(),
      setting,
      meter: null,
      atmospheres: [
        {
          id: aLitBody().body,
          tables,
          camera: {
            positionM: vec3(0, 0, 1e8),
            orientation: IDENTITY_QUATERNION,
            fovXRad: FOV_X_RAD,
            viewport: SIZE,
          },
          toBodyFixed: IDENTITY_QUATERNION,
        },
      ],
    });
    // The view, on the low setting, has drawn its frame, its air's tables built.
    const view = new PhotorealRenderer(engine, "view");
    const low = airAt("low");
    await view.prepare(SIZE, "low", low.role, low.camera);
    view.render(null, low);
    const asked: QualitySetting[] = [];
    const drawn: DrawnPhotorealFrame = {
      ...drawnOf(low),
      remake: (ask) => {
        asked.push(ask.setting);
        return airAt(ask.setting);
      },
    };
    const stills = new PhotorealStills();
    stills.attach(viewOver(engine, () => drawn));
    const scene = sceneOf(stills);
    const renderer = stills.renderer(scene);
    await renderer.prepare();
    const made = { ...engine.counts };
    const before = engine.targetFrames.length;
    submitAll(renderer);
    const composites = engine.targetFrames
      .slice(before)
      .filter((frame) => frame.label === ATMOSPHERE_COMPOSITE_PASS).length;
    expect([
      asked,
      scene.request.setting,
      composites > 0,
      engine.counts.materials - made.materials,
      engine.counts.kernels - made.kernels,
    ]).toEqual([["high"], "high", true, 0, 0]);
    renderer.dispose();
    view.dispose();
    tables.dispose();
  });

  it("draws a frame with air whole, in one tile of the still's size, its read-back tiles its own (R11.T10.e.2)", async () => {
    const engine = await countingRenderEngine();
    const base = viewFrame();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6.371e6);
    const frame: PhotorealFrame = {
      ...base,
      atmospheres: [
        {
          id: aLitBody().body,
          tables,
          camera: {
            positionM: vec3(0, 0, 1e8),
            orientation: IDENTITY_QUATERNION,
            fovXRad: FOV_X_RAD,
            viewport: SIZE,
          },
          toBodyFixed: IDENTITY_QUATERNION,
        },
      ],
    };
    const renderer = new PhotorealStillRenderer(engine, frame, {
      tileLimitPx: 64,
      samples: 2,
      onUnitTimes: () => undefined,
      onDispose: () => undefined,
    });
    expect([renderer.units, renderer.tiling.tiles.length]).toEqual([
      ["scene 0", "scene 0", "bloom 0", "tonemap 0"],
      4,
    ]);
    renderer.dispose();
    tables.dispose();
  });

  it("tone-maps the assembled image through P into its rgba8 image, its bloom over the whole", async () => {
    const engine = await countingRenderEngine();
    const renderer = await prepared(engine, 1);
    submitAll(renderer);
    const tone = stillPasses(engine).find((frame) => frame.label === "still tonemap");
    const p = perspectiveReversedInfinite(FOV_X_RAD, SIZE.widthPx / SIZE.heightPx, NEAR_PLANE_M);
    expect([
      tone?.encoding,
      Array.from(tone?.projection ?? []),
      engine.targetFrames.some((frame) => frame.label === "bloom"),
    ]).toEqual(["in-pass", Array.from(p), true]);
    renderer.dispose();
  });

  it("reads each tile back from its image, its rows padded to the tile's aligned rows", async () => {
    const engine = await countingRenderEngine();
    const rects: TexelRect[] = [];
    vi.spyOn(engine, "readTexture").mockImplementation((...args: unknown[]) => {
      const rect = args[2];
      if (!isTexelRect(rect)) {
        throw new Error("a tile is read by its rect");
      }
      rects.push(rect);
      return Promise.resolve(new Uint8Array(rect.width * rect.height * 4).fill(7).buffer);
    });
    const renderer = await prepared(engine, 1);
    submitAll(renderer);
    const tiling = planTiles(SIZE, 64, "high");
    const texels = await Promise.all(tiling.tiles.map((tile) => renderer.readTile(tile)));
    const last = tiling.tiles.at(-1);
    const lastTexels = texels.at(-1);
    expect([
      rects,
      texels.map((bytes, i) => bytes.byteLength === (tiling.tiles[i]?.bytesPerRow ?? 0) * 35),
      // The last column's 36 px of the still, then its padding.
      [lastTexels?.[36 * 4 - 1], lastTexels?.[36 * 4], last?.bytesPerRow],
    ]).toEqual([
      [
        { x: 0, y: 0, width: 64, height: 35 },
        { x: 64, y: 0, width: 36, height: 35 },
        { x: 0, y: 35, width: 64, height: 35 },
        { x: 64, y: 35, width: 36, height: 35 },
      ],
      [true, true, true, true],
      [7, 0, 256],
    ]);
    renderer.dispose();
  });

  it("times each unit as the sum of its resolves, under its label, once all have resolved", async () => {
    const engine = await countingRenderEngine();
    let listener: ((times: PassTimes) => void) | null = null;
    vi.spyOn(engine, "onPassTimes").mockImplementation((each) => {
      listener = each;
      return () => {
        listener = null;
      };
    });
    // Each target render one resolve, as the engine numbers them.
    vi.spyOn(engine, "passTimesFrame", "get").mockImplementation(() => engine.targetFrames.length);
    const timed: MeasuredPass[] = [];
    const renderer = await prepared(engine, 1, (passes) => {
      timed.push(...passes);
    });
    const first = engine.targetFrames.length;
    renderer.submit(0, 1);
    const sampleResolves = engine.targetFrames.length - first;
    const resolve = (frame: number): void => {
      listener?.({ frame, timer: "full", passes: [timedPass("any", 1_000)] });
    };
    for (let frame = first + 1; frame < first + sampleResolves; frame += 1) {
      resolve(frame);
    }
    const beforeLast = timed.length;
    resolve(first + sampleResolves);
    expect([beforeLast, timed]).toEqual([0, [{ label: "scene 0", ns: sampleResolves * 1_000 }]]);
    renderer.dispose();
  });

  it("releases everything it made at its disposal", async () => {
    const engine = await countingRenderEngine();
    const before = engine.live;
    const renderer = await prepared(engine);
    submitAll(renderer);
    renderer.dispose();
    const { materials, meshes, buffers } = engine.live;
    expect({ materials, meshes, buffers }).toEqual({
      materials: before.materials,
      meshes: before.meshes,
      buffers: before.buffers,
    });
  });

  it("makes a still end to end through the job on the engine's queue: five empty submits, one batch in flight", async () => {
    const engine = await countingRenderEngine();
    vi.spyOn(engine, "readTexture").mockImplementation((...args: unknown[]) => {
      const rect = args[2];
      return Promise.resolve(new ArrayBuffer(isTexelRect(rect) ? rect.width * rect.height * 4 : 0));
    });
    let nowNs = 0;
    const stills = new PhotorealStills({
      samples: 2,
      nextFrame: () => {
        nowNs += 16_683_350;
        return Promise.resolve();
      },
      nowNs: () => nowNs,
    });
    const drawn: DrawnPhotorealFrame = drawnOf(viewFrame());
    stills.attach(viewOver(engine, () => drawn));
    const scene = sceneOf(stills);
    const encoded: ViewSize[] = [];
    const outcome = await runStillJob({
      device: stills.device,
      renderer: stills.renderer(scene),
      request: scene.request,
      setting: stills.setting,
      frameIntervalNs: stills.frameIntervalNs(),
      encode: (_pixels, size) => {
        encoded.push(size);
        return Promise.resolve(new Uint8Array([1]));
      },
      save: (_png, label) =>
        Promise.resolve({ kind: "saved", still: { id: "20261118T000000000Z-00000000", label } }),
      onStep: () => undefined,
    });
    expect([outcome.kind, encoded, engine.inner.emptySubmits, engine.live.materials]).toEqual([
      "saved",
      [SIZE],
      5,
      0,
    ]);
  });

  it("runs a still to its end on the engine it froze, its view detached meanwhile (the display hidden)", async () => {
    const engine = await countingRenderEngine();
    vi.spyOn(engine, "readTexture").mockImplementation((...args: unknown[]) => {
      const rect = args[2];
      return Promise.resolve(new ArrayBuffer(isTexelRect(rect) ? rect.width * rect.height * 4 : 0));
    });
    const stills = new PhotorealStills({ nextFrame: () => Promise.resolve(), nowNs: () => 0 });
    const detach = stills.attach(viewOver(engine, () => drawnOf(viewFrame())));
    const scene = sceneOf(stills);
    const renderer = stills.renderer(scene);
    detach();
    const outcome = await runStillJob({
      device: stills.device,
      renderer,
      request: scene.request,
      setting: stills.setting,
      frameIntervalNs: stills.frameIntervalNs(),
      encode: () => Promise.resolve(new Uint8Array([1])),
      save: (_png, label) =>
        Promise.resolve({ kind: "saved", still: { id: "20261118T000000000Z-00000000", label } }),
      onStep: () => undefined,
    });
    expect([outcome.kind, engine.inner.emptySubmits, stills.device.timer]).toEqual([
      "saved",
      5,
      "absent",
    ]);
  });
});
