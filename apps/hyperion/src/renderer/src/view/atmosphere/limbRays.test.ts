import { afterEach, describe, expect, it, vi } from "vitest";

import { saturnBody, saturnClassMedium, SATURN_SPHEROID } from "../../test/atmosphereFixtures";
import { boundNames, countingRenderEngine } from "../../test/countingRenderEngine";
import { wgslFunctions, wgslReached } from "../../test/atmosphereWgsl";
import { lookAlong } from "../camera/quaternion";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { RenderEngine } from "../engine/types";
import { kernelBindings } from "../engine/webgpu/compute";
import {
  type AtmosphereCamera,
  type AtmosphereScene,
  type AtmosphereSettings,
  createAtmosphereFrameKernels,
  HillaireAtmosphere,
  LIMB_DISPATCH_KERNEL,
  LIMB_KERNELS,
  LIMB_RESOLVE_KERNEL,
  PER_PIXEL_LEVERS,
  RAY_MARCH_CENTRE_ONE_BAND_KERNEL,
  RAY_MARCH_CENTRE_SPECIAL_KERNEL,
  RAY_MARCH_LIMB_KERNEL,
  RAY_MARCH_LIMB_SPECIAL_KERNEL,
  RAY_MARCH_PACKED_KERNEL,
  RAY_MARCH_PACKED_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_SPECIAL_KERNEL,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import { R05_EARTH_REFERENCE, referenceSunIlluminanceLx } from "./earth";
import { AtmosphereTables, NO_ABSORBER_CURVES } from "./tables";

/** WGS 84 (NIMA TR8350.2): a = 6,378,137 m, f = 1 ÷ 298.257223563. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};

/** From 10⁸ m over the equator, looking at the centre: the Earth's limb a few texels across. */
const DISTANT: AtmosphereCamera = {
  positionM: { x: 1e8, y: 0, z: 0 },
  orientation: lookAlong({ x: -1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
  fovXRad: Math.PI / 3,
  viewport: { widthPx: 320, heightPx: 180 },
};

const SCENE: AtmosphereScene = {
  colour: { kind: "texture", name: "terrain colour" },
  depth: { kind: "texture", name: "terrain depth" },
  nearM: 0.1,
  exposureScale: 1e-4,
};

const NOON: SunState = {
  directionBodyFixed: { x: 1, y: 0, z: 0 },
  illuminanceLx: referenceSunIlluminanceLx(),
  angularRadiusRad: 0.004_65,
};

/** The view's levers with the limb's node rays packed, over `marchCover`. */
function packed(marchCover: AtmosphereSettings["marchCover"]): AtmosphereSettings {
  return { ...PER_PIXEL_LEVERS, marchCover, limbRays: "packed" };
}

/** A view's atmosphere of R05's Earth on WGS 84 over the device's shared kernels, under `levers`. */
async function viewOf(levers: AtmosphereSettings): Promise<{
  readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
  readonly view: HillaireAtmosphere;
}> {
  const engine = await countingRenderEngine();
  const kernels = await createAtmosphereFrameKernels(engine);
  const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
  return {
    engine,
    view: HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels, levers),
  };
}

/**
 * {@link viewOf}'s view, its first frame drawn from {@link DISTANT} and the packed kernels it asked
 * for made in the background, under the fake clock, and the engine's records cleared after.
 */
async function readyViewOf(levers: AtmosphereSettings): Promise<{
  readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
  readonly view: HillaireAtmosphere;
}> {
  vi.useFakeTimers();
  const made = await viewOf(levers);
  made.view.drawFrame(DISTANT, [NOON], SCENE);
  await vi.advanceTimersByTimeAsync(0);
  made.engine.dispatched.length = 0;
  made.engine.writes.length = 0;
  return made;
}

/** The march's dispatches among `engine`'s: every kernel of the march's, packed ones among them. */
function marchesOf(
  engine: Awaited<ReturnType<typeof countingRenderEngine>>,
): ReadonlyArray<(typeof engine.dispatched)[number]> {
  return engine.dispatched.filter((d) => d.kernel.startsWith("atmosphere ray march"));
}

/** The bindings `wgsl` declares that neither its entry point nor any function it reaches names. */
function unreachedBindings(wgsl: string): string[] {
  const functions = wgslFunctions(wgsl);
  const bodies = [...wgslReached(wgsl, ["main"])].map((name) => functions.get(name)?.body ?? "");
  return [...kernelBindings(wgsl).keys()].filter(
    (name) => !bodies.some((body) => new RegExp(`\\b${name}\\b`, "u").test(body)),
  );
}

/** The calls of the march's one ray (`marchRay`) in the functions `main` reaches. */
function marchRayCalls(wgsl: string): number {
  const functions = wgslFunctions(wgsl);
  return [...wgslReached(wgsl, ["main"])]
    .map((name) => [...(functions.get(name)?.body ?? "").matchAll(/\bmarchRay\(/gu)].length)
    .reduce((sum, calls) => sum + calls, 0);
}

/** Whether `wgsl` declares either of the march's targets. */
function declared(wgsl: string): boolean {
  return ["rayMarchOut", "rayMarchTransmittanceOut"].some((name) => kernelBindings(wgsl).has(name));
}

describe("the far limb's node rays, packed (plan R08, R08.T8.d)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  // The engine's compute layouts are "auto": a declared binding the entry point never reaches drops
  // out of the layout, and the bind group, which takes every declared binding, fails.
  it.each(
    [
      ...LIMB_KERNELS,
      RAY_MARCH_CENTRE_SPECIAL_KERNEL,
      RAY_MARCH_CENTRE_ONE_BAND_KERNEL,
      RAY_MARCH_SLOTS_CENTRE_SPECIAL_KERNEL,
      RAY_MARCH_SLOTS_CENTRE_ONE_BAND_KERNEL,
    ].map((pair) => [pair.name, pair.reference]),
  )("reaches every binding %s declares from its entry point", (_name, source) => {
    expect(unreachedBindings(source)).toEqual([]);
  });

  // SwiftShader inlines every call, and its GPU process crashed on a march emitted twice in one
  // entry point (R08.T8.d): the packed entry point and the node pass march through one call each.
  it("marches one ray through one call site in the packed entry point and in the node pass", () => {
    const marching = LIMB_KERNELS.filter(
      (pair) => pair !== LIMB_RESOLVE_KERNEL && pair !== LIMB_DISPATCH_KERNEL,
    );
    expect(marching).toHaveLength(12);
    expect(marching.map((pair) => marchRayCalls(pair.reference))).toEqual(marching.map(() => 1));
    expect(
      [LIMB_RESOLVE_KERNEL, LIMB_DISPATCH_KERNEL].map((pair) => marchRayCalls(pair.reference)),
    ).toEqual([0, 0]);
  });

  it("leaves the node pass without the march's targets, which the resolve alone writes", () => {
    expect(
      [RAY_MARCH_PACKED_KERNEL, RAY_MARCH_LIMB_KERNEL, LIMB_RESOLVE_KERNEL].map((pair) =>
        declared(pair.reference),
      ),
    ).toEqual([true, false, true]);
  });

  it("is catalogued, so that the smoke page compiles every packed kernel", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(expect.arrayContaining(LIMB_KERNELS.map((pair) => pair.name)));
  });

  it("marches the area by the packed entry point, then the lists' count, the node pass and the resolve, each bound as it declares", async () => {
    const { engine, view } = await readyViewOf(packed("area"));
    engine.dispatched.length = 0;
    engine.writes.length = 0;
    view.drawFrame(DISTANT, [NOON], SCENE);
    const marches = marchesOf(engine);
    expect(
      marches.map((d) => [
        d.kernel,
        "buffer" in d.workgroups ? d.workgroups.offsetBytes : "direct",
      ]),
    ).toEqual([
      [RAY_MARCH_PACKED_SPECIAL_KERNEL.name, "direct"],
      [LIMB_DISPATCH_KERNEL.name, "direct"],
      [RAY_MARCH_LIMB_SPECIAL_KERNEL.name, 0],
      [LIMB_RESOLVE_KERNEL.name, 12],
    ]);
    const kernelOf = new Map(
      [
        RAY_MARCH_PACKED_SPECIAL_KERNEL,
        LIMB_DISPATCH_KERNEL,
        RAY_MARCH_LIMB_SPECIAL_KERNEL,
        LIMB_RESOLVE_KERNEL,
      ].map((pair) => [pair.name, pair] as const),
    );
    expect(marches.map((d) => boundNames(d.bindings))).toEqual(
      marches.map((d) =>
        [...kernelBindings(kernelOf.get(d.kernel)?.reference ?? "").keys()].toSorted(),
      ),
    );
    // The lists' counts cleared before the packed entry point, which reads them, and each list one
    // buffer through the four.
    const cleared = engine.writes.findIndex(
      (write) => write.buffer === "atmosphere far limb's counts",
    );
    expect(cleared).toBeGreaterThanOrEqual(0);
    expect(marches[0]?.writesBefore).toBeGreaterThan(cleared);
    const rays = marches.map((d) => d.bindings.buffers["limbRays"]);
    expect(new Set(rays).size).toBe(1);
    expect(marches[2]?.workgroups).toMatchObject({
      buffer: marches[1]?.bindings.buffers["limbDispatch"],
    });
  });

  it("takes the cull's centre ray first under `tiles`, and marches per texel on the reference path", async () => {
    const { engine, view } = await readyViewOf(packed("tiles"));
    engine.dispatched.length = 0;
    view.drawFrame(DISTANT, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([
      RAY_MARCH_CENTRE_SPECIAL_KERNEL.name,
      RAY_MARCH_PACKED_SPECIAL_KERNEL.name,
      LIMB_DISPATCH_KERNEL.name,
      RAY_MARCH_LIMB_SPECIAL_KERNEL.name,
      LIMB_RESOLVE_KERNEL.name,
    ]);
    view.setMarchCover("whole frame");
    engine.dispatched.length = 0;
    view.drawFrame(DISTANT, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual(["atmosphere ray march"]);
  });

  it("marches per texel at its first frame while its packed kernels are made in the background, and packs from the next", async () => {
    vi.useFakeTimers();
    const { engine, view } = await viewOf(packed("area"));
    view.drawFrame(DISTANT, [NOON], SCENE);
    const first = marchesOf(engine).map((d) => d.kernel);
    const asked = engine.kernelPairs
      .filter((pair) => pair.name.includes("limb"))
      .map((p) => p.name);
    await vi.advanceTimersByTimeAsync(0);
    engine.dispatched.length = 0;
    view.drawFrame(DISTANT, [NOON], SCENE);
    expect({
      first,
      asked: asked.toSorted(),
      next: marchesOf(engine).map((d) => d.kernel),
    }).toEqual({
      first: ["atmosphere ray march for one band, edge-free"],
      asked: [
        LIMB_DISPATCH_KERNEL.name,
        RAY_MARCH_LIMB_SPECIAL_KERNEL.name,
        RAY_MARCH_PACKED_SPECIAL_KERNEL.name,
        LIMB_RESOLVE_KERNEL.name,
      ].toSorted(),
      next: [
        RAY_MARCH_PACKED_SPECIAL_KERNEL.name,
        LIMB_DISPATCH_KERNEL.name,
        RAY_MARCH_LIMB_SPECIAL_KERNEL.name,
        LIMB_RESOLVE_KERNEL.name,
      ],
    });
  });

  it("asks for no packed kernel from inside the air, where no footprint blends", async () => {
    vi.useFakeTimers();
    const { engine, view } = await viewOf(packed("area"));
    const inside: AtmosphereCamera = {
      ...DISTANT,
      positionM: { x: WGS84.equatorialRadiusM + 2_000, y: 0, z: 0 },
    };
    view.drawFrame(inside, [NOON], SCENE);
    await vi.advanceTimersByTimeAsync(0);
    view.drawFrame(inside, [NOON], SCENE);
    expect(engine.kernelPairs.filter((pair) => pair.name.includes("limb"))).toEqual([]);
  });

  it("marches per texel, and makes no packed kernel, on the way back", async () => {
    const { engine, view } = await viewOf(PER_PIXEL_LEVERS);
    engine.dispatched.length = 0;
    view.drawFrame(DISTANT, [NOON], SCENE);
    const names = new Set(LIMB_KERNELS.map((pair) => pair.name));
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([
      "atmosphere ray march for one band, edge-free",
    ]);
    expect(engine.kernelPairs.filter((pair) => names.has(pair.name))).toEqual([]);
  });

  it("packs a general body's limb by the general forms", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(
      engine,
      saturnClassMedium(),
      tableRadiusM(SATURN_SPHEROID),
      NO_ABSORBER_CURVES,
      saturnBody(),
    );
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels, packed("area"));
    const far = { ...DISTANT, positionM: { x: 1e9, y: 0, z: 0 } };
    vi.useFakeTimers();
    view.drawFrame(far, [NOON], SCENE);
    await vi.advanceTimersByTimeAsync(0);
    engine.dispatched.length = 0;
    view.drawFrame(far, [NOON], SCENE);
    expect(marchesOf(engine).map((d) => d.kernel)).toEqual([
      RAY_MARCH_PACKED_KERNEL.name,
      LIMB_DISPATCH_KERNEL.name,
      RAY_MARCH_LIMB_KERNEL.name,
      LIMB_RESOLVE_KERNEL.name,
    ]);
  });

  it("grows its lists for the frames after one whose counts passed their room, read back at times", async () => {
    // Each read-back settles on the queue's turns, which the fake clock runs.
    const { engine, view } = await readyViewOf(packed("area"));
    let reported = new Uint32Array([0, 0]);
    const read: string[] = [];
    const asEngine: RenderEngine = engine;
    vi.spyOn(asEngine, "readBuffer").mockImplementation((buffer) => {
      read.push(buffer.name);
      return Promise.resolve(reported.slice().buffer);
    });
    const roomOf = (): number | undefined =>
      marchesOf(engine).at(-1)?.bindings.buffers["limbTexels"]?.bytes;
    view.drawFrame(DISTANT, [NOON], SCENE);
    const first = roomOf() ?? 0;
    expect(first).toBeGreaterThan(0);
    // The first frame's counts read back; frames before the next read leave the lists as they are.
    await vi.advanceTimersByTimeAsync(0);
    const room = first / 48;
    reported = new Uint32Array([room + 1, 0]);
    for (let i = 0; i < 29; i += 1) {
      view.drawFrame(DISTANT, [NOON], SCENE);
    }
    expect([read.length, roomOf()]).toEqual([1, first]);
    // The thirtieth after reads the counts that passed the room, and the frame after grows.
    view.drawFrame(DISTANT, [NOON], SCENE);
    await vi.advanceTimersByTimeAsync(0);
    view.drawFrame(DISTANT, [NOON], SCENE);
    expect(read).toEqual(["atmosphere far limb's counts", "atmosphere far limb's counts"]);
    expect(roomOf()).toBe(Math.ceil((room + 1) * 1.25) * 48);
  });
});
