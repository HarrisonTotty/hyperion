import { describe, expect, it } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { FakeRenderEngine } from "../../test/fakeRenderEngine";
import { decodedSky, skyPayload } from "../../test/skyFixtures";
import type {
  BufferHandle,
  ComputeHandle,
  PointSplatHandle,
  RenderEngine,
  TextureHandle,
} from "../engine/types";
import type { BufferSpec, TextureSpec } from "../engine/memory";
import { bakeSourceOf } from "./bake";
import { type CubeRequest, SkyCubeCache } from "./cache";

/** An engine without float32-blendable that bakes on the CPU and records cubes made and released. */
async function countingCubes(): Promise<{
  engine: RenderEngine;
  made: string[];
  released: string[];
  restore: () => void;
}> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const device = adapter.devices.at(0);
  if (device === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const made: string[] = [];
  const released: string[] = [];
  let next = 0;
  const restored: Array<() => void> = [];
  const engine = Object.assign(new FakeRenderEngine(device), {
    onRestored: (listener: () => void): (() => void) => {
      restored.push(listener);
      return () => undefined;
    },
    createPackedCube: (): TextureHandle => {
      next += 1;
      const name = `cube ${String(next)}`;
      made.push(name);
      return { kind: "texture", name };
    },
    writePackedCubeLevel: (): void => undefined,
    createBuffer: (spec: BufferSpec): BufferHandle => ({
      kind: "buffer",
      name: spec.name,
      bytes: spec.bytes,
    }),
    writeBuffer: (): void => undefined,
    releaseBuffer: (): void => undefined,
    releaseTexture: (texture: TextureHandle): void => {
      released.push(texture.name);
    },
  });
  return {
    engine,
    made,
    released,
    restore: () => {
      for (const listener of restored) {
        listener();
      }
    },
  };
}

/**
 * An engine with float32-blendable that bakes on the GPU and records cubes made and released: its
 * bakes' splats and levels run in tasks after their uploads' frame.
 */
async function gpuCubes(): Promise<{
  engine: RenderEngine;
  made: string[];
  released: string[];
  /** Each buffer written, by name, in order. */
  written: string[];
  /** Each buffer released, by name, in order. */
  releasedBuffers: string[];
  restore: () => void;
}> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: ["float32-blendable"] });
  await adapter.requestDevice({ requiredFeatures: ["float32-blendable"] });
  const device = adapter.devices.at(0);
  if (device === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const made: string[] = [];
  const released: string[] = [];
  const written: string[] = [];
  const releasedBuffers: string[] = [];
  const restored: Array<() => void> = [];
  let next = 0;
  const engine = Object.assign(new FakeRenderEngine(device), {
    onRestored: (listener: () => void): (() => void) => {
      restored.push(listener);
      return () => undefined;
    },
    createCompute: (pair: { readonly name: string }): ComputeHandle => ({
      kind: "compute",
      name: pair.name,
      path: "reference",
    }),
    createPointSplat: (): PointSplatHandle => ({
      draw: (): void => undefined,
      dispose: (): void => undefined,
    }),
    createPackedCube: (): TextureHandle => {
      next += 1;
      const name = `cube ${String(next)}`;
      made.push(name);
      return { kind: "texture", name };
    },
    createTexture: (spec: TextureSpec): TextureHandle => ({ kind: "texture", name: spec.name }),
    createBuffer: (spec: BufferSpec): BufferHandle => ({
      kind: "buffer",
      name: spec.name,
      bytes: spec.bytes,
    }),
    writeBuffer: (buffer: BufferHandle): void => {
      written.push(buffer.name);
    },
    dispatch: (): void => undefined,
    writePackedCubeLevelFromBuffer: (): void => undefined,
    releaseBuffer: (buffer: BufferHandle): void => {
      releasedBuffers.push(buffer.name);
    },
    releaseTexture: (texture: TextureHandle): void => {
      if (texture.name.startsWith("cube ")) {
        released.push(texture.name);
      }
    },
  });
  return {
    engine,
    made,
    released,
    written,
    releasedBuffers,
    restore: () => {
      for (const listener of restored) {
        listener();
      }
    },
  };
}

/** A decoded sky of two stars, both baked; each call is another sky, as another arrival's is. */
function aSky() {
  return decodedSky(
    skyPayload(
      [
        { direction: [1, 0, 0], distanceLy: 0.05, vMag: 9 },
        { direction: [0, 1, 0], distanceLy: 400, vMag: 9 },
      ],
      2,
      null,
    ),
    2,
  ).stars;
}

/** The sky the views near one another share. */
const STARS = aSky();

/** A view's request for a sky's two stars. */
function requestOf(stars = STARS, faceSizePx = 4): CubeRequest {
  return {
    stars,
    baked: Uint32Array.from([0, 1]),
    bakeSource: () =>
      bakeSourceOf({
        directions: stars.directions,
        illuminanceLx: new Float32Array(6).fill(1e-9),
        faceSizePx,
        name: "sky cube",
      }),
  };
}

describe("SkyCubeCache", () => {
  it("shares one cube between two views near one another", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    const a = cache.acquire("cockpit", requestOf());
    const b = cache.acquire("instrument", requestOf());
    expect(a).toBe(b);
    expect([made.length, cache.size]).toEqual([1, 1]);
  });

  it("gives two views far apart, each with a sky of its own place, a cube each", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    cache.acquire("far camera", requestOf(aSky()));
    expect([made.length, cache.size]).toEqual([2, 2]);
  });

  it("releases a view's cube with its last view", async () => {
    const { engine, released } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    cache.acquire("instrument", requestOf());
    cache.release("cockpit");
    expect(released).toEqual([]);
    cache.release("instrument");
    expect([released, cache.size]).toEqual([["cube 1"], 0]);
  });

  it("re-bakes a view's cube when its new sky arrives, letting the old one go", async () => {
    const { engine, made, released } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    cache.acquire("cockpit", requestOf(aSky()));
    expect([made, released]).toEqual([["cube 1", "cube 2"], ["cube 1"]]);
  });

  it("bakes nothing for a view with no baked stars", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    expect(cache.acquire("cockpit", { ...requestOf(), baked: new Uint32Array(0) })).toBeNull();
    expect(made).toEqual([]);
  });

  it("bakes again after a device loss's restore, the old cubes having died with the device", async () => {
    const { engine, made, released, restore } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("cockpit", requestOf());
    restore();
    cache.acquire("cockpit", requestOf());
    expect([made, released, cache.size]).toEqual([["cube 1", "cube 2"], [], 1]);
  });

  it("keeps a disposed view's cube as its spare, which a view of the same sky takes up again (plan R15)", async () => {
    const { engine, made, released } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    const cube = cache.acquire("view", requestOf());
    cache.letGo("view");
    const held = cache.size;
    // A new stage's view, its selection culled again: the same stars, in another array.
    const again = cache.acquire("view", { ...requestOf(), baked: Uint32Array.from([0, 1]) });
    expect([again === cube, made, released, held, cache.size]).toEqual([
      true,
      ["cube 1"],
      [],
      1,
      1,
    ]);
  });

  it("releases its spare before another sky's bake, and with another spare", async () => {
    const { engine, made, released } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("view", requestOf());
    cache.letGo("view");
    cache.acquire("view", requestOf(aSky()));
    const beforeBake = [...released];
    cache.letGo("view");
    cache.acquire("other", requestOf());
    cache.letGo("other");
    expect([beforeBake, made, released, cache.size]).toEqual([
      ["cube 1"],
      ["cube 1", "cube 2", "cube 3"],
      ["cube 1", "cube 2"],
      1,
    ]);
  });

  it("forgets its spare at a device loss's restore", async () => {
    const { engine, made, released, restore } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    cache.acquire("view", requestOf());
    cache.letGo("view");
    restore();
    cache.acquire("view", requestOf());
    expect([made, released, cache.size]).toEqual([["cube 1", "cube 2"], [], 1]);
  });

  it("keeps a view's cube across frames that hand it the same stars again", async () => {
    const { engine, made } = await countingCubes();
    const cache = new SkyCubeCache(engine);
    const request = requestOf();
    for (let frame = 0; frame < 10; frame += 1) {
      cache.acquire("cockpit", { ...request, baked: Uint32Array.from(request.baked) });
    }
    expect(made).toEqual(["cube 1"]);
  });
});

/**
 * A cache whose bake tasks wait to be run, on a clock that passes a task's budget at each read,
 * so that each task runs one step.
 */
function steppedCache(engine: RenderEngine): {
  cache: SkyCubeCache;
  tasks: Array<() => void>;
  runTasks: () => number;
} {
  const tasks: Array<() => void> = [];
  let clockMs = 0;
  const cache = new SkyCubeCache(engine, {
    schedule: (task) => {
      tasks.push(task);
    },
    now: () => {
      clockMs += 10;
      return clockMs;
    },
  });
  return {
    cache,
    tasks,
    runTasks: () => {
      let ran = 0;
      for (let task = tasks.shift(); task !== undefined; task = tasks.shift()) {
        task();
        ran += 1;
      }
      return ran;
    },
  };
}

/** Asks for a request at each frame, the tasks run between, until its cube is taken up. */
function bakeThrough(
  cache: SkyCubeCache,
  view: string,
  request: CubeRequest,
  runTasks: () => number,
): void {
  for (let frame = 0; frame < 100; frame += 1) {
    if (cache.want(view, request).kind !== "baking") {
      return;
    }
    runTasks();
  }
  throw new Error("the bake did not end in 100 frames");
}

describe("SkyCubeCache's bakes in the tasks after a frame (lane perf-long-tasks-3)", () => {
  it("bakes a view's first cube in tasks of its own, its solid angles in a frame after the one that asks and its other uploads in the next", async () => {
    const { engine, made, written } = await gpuCubes();
    const { cache, tasks, runTasks } = steppedCache(engine);
    expect(cache.want("cockpit", requestOf())).toEqual({ kind: "baking" });
    expect([made, tasks.length]).toEqual([[], 1]);
    // The stars' layout, a task a step, and then nothing until a frame asks for the uploads.
    expect(runTasks()).toBe(2);
    expect(written).toEqual([]);
    expect(cache.want("cockpit", requestOf())).toEqual({ kind: "baking" });
    // The held solid angles alone in that frame (perf-long-tasks-4), and nothing until the next.
    runTasks();
    expect(written).toEqual(["sky bake 4² solid angles"]);
    expect(cache.want("cockpit", requestOf())).toEqual({ kind: "baking" });
    expect(written).toEqual(["sky bake 4² solid angles", "sky cube points", "sky cube peak"]);
    // Then its splats and levels in tasks: six faces' peaks, six faces' tiles and levels.
    expect(runTasks()).toBeGreaterThanOrEqual(6 * 2);
    const state = cache.want("cockpit", requestOf());
    expect(state.kind === "baked" && state.cube !== null).toBe(true);
    expect([made, cache.baking]).toEqual([["cube 1"], 0]);
  });

  it("bakes on the CPU in the frame after its stars' layout, and draws it in that frame", async () => {
    const { engine, made } = await countingCubes();
    const { cache, runTasks } = steppedCache(engine);
    cache.want("cockpit", requestOf());
    runTasks();
    expect(made).toEqual([]);
    const state = cache.want("cockpit", requestOf());
    expect(state.kind === "baked" && state.cube !== null).toBe(true);
    expect(made).toEqual(["cube 1"]);
  });

  it("keeps a view's cube while its new sky's is baked, and lets it go as the new one is taken up", async () => {
    const { engine, made, released } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    bakeThrough(cache, "cockpit", requestOf(), runTasks);
    const first = cache.held("cockpit");
    expect(first).not.toBeNull();
    const next = requestOf(aSky());
    expect(cache.want("cockpit", next)).toEqual({ kind: "baking" });
    runTasks();
    expect(cache.want("cockpit", next)).toEqual({ kind: "baking" });
    runTasks();
    // Baked, and not yet taken up: the view still holds the old cube, which its frames draw.
    expect([cache.held("cockpit"), made, released]).toEqual([first, ["cube 1", "cube 2"], []]);
    const state = cache.want("cockpit", next);
    expect(state.kind === "baked" && state.cube === cache.held("cockpit")).toBe(true);
    expect([released, cache.size]).toEqual([["cube 1"], 1]);
  });

  it("shares one bake between two views of one sky", async () => {
    const { engine, made } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    cache.want("cockpit", requestOf());
    cache.want("instrument", requestOf());
    runTasks();
    // The held solid angles' frame, then the other uploads' (perf-long-tasks-4).
    cache.want("instrument", requestOf());
    cache.want("cockpit", requestOf());
    runTasks();
    const a = cache.want("cockpit", requestOf());
    const b = cache.want("instrument", requestOf());
    expect(a.kind === "baked" && b.kind === "baked" && a.cube === b.cube).toBe(true);
    expect(made).toEqual(["cube 1"]);
  });

  it("stops a bake that a newer sky replaces before it ends", async () => {
    const { engine, made } = await countingCubes();
    const { cache, tasks, runTasks } = steppedCache(engine);
    cache.want("cockpit", requestOf());
    // One step, the stars' layout: the bake is under way.
    tasks.shift()?.();
    expect(cache.baking).toBe(1);
    const next = requestOf(aSky());
    cache.want("cockpit", next);
    runTasks();
    expect(cache.want("cockpit", next).kind).toBe("baked");
    // The first bake made no cube: only the newer sky's was made.
    expect([made, cache.size]).toEqual([["cube 1"], 1]);
  });

  it("stops a bake whose last view is released", async () => {
    const { engine, made } = await countingCubes();
    const { cache, tasks, runTasks } = steppedCache(engine);
    cache.want("cockpit", requestOf());
    tasks.shift()?.();
    cache.release("cockpit");
    runTasks();
    expect([made, cache.baking, cache.size]).toEqual([[], 0, 0]);
  });

  it("keeps a bake finished after its views moved on as the spare, which the next bake releases", async () => {
    const { engine, made, released } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    bakeThrough(cache, "cockpit", requestOf(), runTasks);
    const next = requestOf(aSky());
    cache.want("cockpit", next);
    runTasks();
    cache.want("cockpit", next);
    runTasks();
    // Back to the sky it holds before taking the new cube up: the new one is the spare.
    expect(cache.want("cockpit", requestOf()).kind).toBe("baked");
    expect([released, cache.size]).toEqual([[], 2]);
    const last = requestOf(aSky());
    cache.want("cockpit", last);
    expect(released).toEqual(["cube 2"]);
    bakeThrough(cache, "cockpit", last, runTasks);
    expect(made).toEqual(["cube 1", "cube 2", "cube 3"]);
  });

  it("says a bake that fails at its view's next ask", async () => {
    const { engine } = await countingCubes();
    const failing = Object.assign(engine, {
      createPackedCube: (): TextureHandle => {
        throw new Error("out of memory for the cube");
      },
    });
    const { cache, runTasks } = steppedCache(failing);
    cache.want("cockpit", requestOf());
    runTasks();
    const state = cache.want("cockpit", requestOf());
    expect(state.kind).toBe("failed");
    expect(state.kind === "failed" && String(state.error)).toMatch(/out of memory/);
    expect([cache.baking, cache.size]).toEqual([0, 0]);
  });

  it("tells each view that waited on a failed bake at its next ask, and bakes it once", async () => {
    const { engine } = await countingCubes();
    let attempts = 0;
    const failing = Object.assign(engine, {
      createPackedCube: (): TextureHandle => {
        attempts += 1;
        throw new Error("out of memory for the cube");
      },
    });
    const { cache, runTasks } = steppedCache(failing);
    cache.want("cockpit", requestOf());
    cache.want("instrument", requestOf());
    runTasks();
    expect([
      cache.want("cockpit", requestOf()).kind,
      cache.want("instrument", requestOf()).kind,
    ]).toEqual(["failed", "failed"]);
    runTasks();
    expect([attempts, cache.baking]).toEqual([1, 0]);
  });

  it("keeps a bake ended for a disposed view as its spare, in place of the cube it held", async () => {
    const { engine, made, released } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    bakeThrough(cache, "view", requestOf(), runTasks);
    const next = requestOf(aSky());
    cache.want("view", next);
    runTasks();
    cache.want("view", next);
    runTasks();
    cache.letGo("view");
    // The newer cube, of the sky the view last asked for, is the spare a remounted stage takes up.
    expect([released, cache.size]).toEqual([["cube 1"], 1]);
    const taken = cache.acquire("remounted", next);
    expect([taken?.cube.name, made]).toEqual(["cube 2", ["cube 1", "cube 2"]]);
  });

  it("forgets a bake under way at a device loss's restore and bakes again at the next ask", async () => {
    const { engine, made, restore } = await countingCubes();
    const { cache, tasks, runTasks } = steppedCache(engine);
    cache.want("cockpit", requestOf());
    tasks.shift()?.();
    restore();
    expect(cache.baking).toBe(0);
    expect(cache.want("cockpit", requestOf())).toEqual({ kind: "baking" });
    runTasks();
    expect(cache.want("cockpit", requestOf()).kind).toBe("baked");
    expect(made).toEqual(["cube 1"]);
  });

  it("finishes a bake under way at once where a view acquires its sky", async () => {
    const { engine, made } = await countingCubes();
    const { cache, tasks } = steppedCache(engine);
    cache.want("cockpit", requestOf());
    tasks.shift()?.();
    const cube = cache.acquire("instrument", requestOf());
    expect(cube).not.toBeNull();
    expect(cache.want("cockpit", requestOf())).toEqual({ kind: "baked", cube });
    expect(made).toEqual(["cube 1"]);
  });
});

/** The writes, or releases, of the held solid angles among `names`. */
function anglesWritten(names: ReadonlyArray<string>): string[] {
  return names.filter((name) => name.endsWith("solid angles"));
}

describe("SkyCubeCache's solid angles, held from bake to bake (lane perf-long-tasks-4)", () => {
  it("re-bakes over the solid angles its first bake wrote, its frame step writing its points alone", async () => {
    const { engine, written, releasedBuffers } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    bakeThrough(cache, "cockpit", requestOf(), runTasks);
    const next = requestOf(aSky());
    expect(cache.want("cockpit", next)).toEqual({ kind: "baking" });
    runTasks();
    const before = written.length;
    // One frame's step, the points' upload, and then the splats and levels in tasks.
    expect(cache.want("cockpit", next)).toEqual({ kind: "baking" });
    expect(written.slice(before)).toEqual(["sky cube points", "sky cube peak"]);
    runTasks();
    expect(cache.want("cockpit", next).kind).toBe("baked");
    expect(anglesWritten(written)).toEqual(["sky bake 4² solid angles"]);
    expect(releasedBuffers).not.toContain("sky bake 4² solid angles");
  });

  it("releases a face size's solid angles once a bake of another size has taken its own", async () => {
    const { engine, written, releasedBuffers } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    bakeThrough(cache, "cockpit", requestOf(), runTasks);
    bakeThrough(cache, "cockpit", requestOf(aSky(), 8), runTasks);
    expect(anglesWritten(written)).toEqual([
      "sky bake 4² solid angles",
      "sky bake 8² solid angles",
    ]);
    expect(anglesWritten(releasedBuffers)).toEqual(["sky bake 4² solid angles"]);
  });

  it("writes them anew after a device loss's restore, the old buffer having died with the device", async () => {
    const { engine, written, releasedBuffers, restore } = await gpuCubes();
    const { cache, runTasks } = steppedCache(engine);
    bakeThrough(cache, "cockpit", requestOf(), runTasks);
    restore();
    bakeThrough(cache, "cockpit", requestOf(), runTasks);
    expect(anglesWritten(written)).toEqual([
      "sky bake 4² solid angles",
      "sky bake 4² solid angles",
    ]);
    expect(anglesWritten(releasedBuffers)).toEqual([]);
  });
});
